use super::*;
use serenity::model::id::GuildId;
use serenity::model::prelude::Mention;
use serenity::model::prelude::UserId;
use serenity::prelude::{Context, Mentionable, SerenityError};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

pub(super) struct TextRedAlertCommand {
    pub(super) actions_history: Arc<Mutex<ActionsHistory>>,
    pub(super) red_alert_handler: Arc<RedAlertHandler>,
    pub(super) l10n: L10n,
}

enum CommonRedAlertResult {
    Empty {
        auto_self_kick_result: RedAlertDeportationResult,
    },
    SingleSuccess {
        is_self_kick: bool,
    },
    SingleNotFound {
        auto_self_kick_result: Option<RedAlertDeportationResult>,
    },
    SingleError {
        error: SerenityError,
        is_self_kick_try: bool,
    },
    Mass {
        results: Vec<RedAlertDeportationResult>,
        auto_self_kick_result: Option<RedAlertDeportationResult>,
    },
}

async fn common_red_alert(
    red_alert_handler: Arc<RedAlertHandler>,
    ctx: &Context,
    guild_id: &GuildId,
    author_user_id: &UserId,
    target_users_ids: &Vec<UserId>,
) -> CommonRedAlertResult {
    let mut red_alert_results = red_alert_handler
        .multiple(ctx, guild_id, target_users_ids)
        .await;
    match red_alert_results.len() {
        0 => CommonRedAlertResult::Empty {
            auto_self_kick_result: red_alert_handler
                .single(ctx, guild_id, author_user_id)
                .await,
        },
        1 => {
            let is_self_kick = *author_user_id == target_users_ids[0];
            match red_alert_results.remove(0) {
                RedAlertDeportationResult::Deported => {
                    CommonRedAlertResult::SingleSuccess { is_self_kick }
                }
                RedAlertDeportationResult::NotFound => CommonRedAlertResult::SingleNotFound {
                    auto_self_kick_result: if !is_self_kick {
                        Some(
                            red_alert_handler
                                .single(ctx, guild_id, author_user_id)
                                .await,
                        )
                    } else {
                        None
                    },
                },
                RedAlertDeportationResult::Error(error) => CommonRedAlertResult::SingleError {
                    error,
                    is_self_kick_try: is_self_kick,
                },
            }
        }
        _ => {
            let mut any_founded = false;
            for red_alert_result in &red_alert_results {
                if red_alert_result.is_not_found() {
                    continue;
                }
                any_founded = true;
                break;
            }
            CommonRedAlertResult::Mass {
                results: red_alert_results,
                auto_self_kick_result: if !any_founded && !target_users_ids.contains(author_user_id)
                {
                    Some(
                        red_alert_handler
                            .single(ctx, guild_id, author_user_id)
                            .await,
                    )
                } else {
                    None
                },
            }
        }
    }
}

#[async_trait]
impl Command for TextRedAlertCommand {
    fn prefix_anchor(&self) -> String {
        self.l10n
            .string("red-alert-command-prefix-anchor", fluent_args![])
    }
    fn help_info(&self) -> Option<HelpInfo> {
        Some(HelpInfo {
            header_suffix: Some(
                self.l10n
                    .string("red-alert-command-header-suffix", fluent_args![]),
            ),
            description: self
                .l10n
                .string("red-alert-command-help-description", fluent_args![]),
        })
    }
    async fn process<'a>(&'a self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let author_id = params.author.id;
        let target_users_ids: Vec<UserId> = params
            .args
            .iter()
            .filter_map(|a| match Mention::from_str(a) {
                Ok(mention) => match mention {
                    Mention::User(user_id) => Some(user_id),
                    Mention::Channel(_) | Mention::Role(_) | Mention::Emoji(_, _) => None,
                },
                Err(_) => a.parse::<u64>().ok().map(UserId),
            })
            .collect();
        let answer_msg = match common_red_alert(
            self.red_alert_handler.clone(),
            &ctx,
            &guild_id,
            &author_id,
            &target_users_ids,
        )
        .await
        {
            CommonRedAlertResult::Empty {
                auto_self_kick_result,
            } => {
                self.actions_history.lock().await.log_history(
                    guild_id,
                    ActionType::TextRedAlert {
                        author_id,
                        target_id: author_id,
                        is_success: auto_self_kick_result.is_deported(),
                    },
                );
                match auto_self_kick_result {
                    RedAlertDeportationResult::Deported => self
                        .l10n
                        .string("red-alert-command-empty-self-success", fluent_args![]),
                    RedAlertDeportationResult::NotFound => self
                        .l10n
                        .string("red-alert-command-empty-self-not-found", fluent_args![]),
                    RedAlertDeportationResult::Error(error) => self.l10n.string(
                        "red-alert-command-empty-self-error",
                        fluent_args![
                            "error" => error.to_string()
                        ],
                    ),
                }
            }
            CommonRedAlertResult::SingleSuccess { is_self_kick } => {
                self.actions_history.lock().await.log_history(
                    guild_id,
                    ActionType::TextRedAlert {
                        author_id,
                        target_id: target_users_ids[0],
                        is_success: true,
                    },
                );
                if is_self_kick {
                    self.l10n
                        .string("red-alert-command-single-self-success", fluent_args![])
                } else {
                    self.l10n.string(
                        "red-alert-command-single-target-success",
                        fluent_args![
                            "user-name" => target_users_ids[0].mention().to_string()
                        ],
                    )
                }
            }
            CommonRedAlertResult::SingleNotFound {
                auto_self_kick_result,
            } => {
                let mut actions_history = self.actions_history.lock().await;
                actions_history.log_history(
                    guild_id,
                    ActionType::TextRedAlert {
                        author_id,
                        target_id: target_users_ids[0],
                        is_success: false,
                    },
                );
                if let Some(self_kick_result) = auto_self_kick_result {
                    actions_history.log_history(
                        guild_id,
                        ActionType::TextRedAlert {
                            author_id,
                            target_id: author_id,
                            is_success: self_kick_result.is_deported(),
                        },
                    );
                    match self_kick_result {
                        RedAlertDeportationResult::Deported => self.l10n.string(
                            "red-alert-command-single-not-found-self-success",
                            fluent_args![],
                        ),
                        RedAlertDeportationResult::NotFound => self.l10n.string(
                            "red-alert-command-single-not-found-self-not-found",
                            fluent_args![],
                        ),
                        RedAlertDeportationResult::Error(error) => self.l10n.string(
                            "red-alert-command-single-not-found-self-error",
                            fluent_args![
                                "error" => error.to_string()
                            ],
                        ),
                    }
                } else {
                    self.l10n
                        .string("red-alert-command-single-not-found-self", fluent_args![])
                }
            }
            CommonRedAlertResult::SingleError {
                error,
                is_self_kick_try,
            } => {
                self.actions_history.lock().await.log_history(
                    guild_id,
                    ActionType::TextRedAlert {
                        author_id,
                        target_id: target_users_ids[0],
                        is_success: false,
                    },
                );
                if is_self_kick_try {
                    self.l10n.string(
                        "red-alert-command-single-self-error",
                        fluent_args![
                            "error" => error.to_string()
                        ],
                    )
                } else {
                    self.l10n.string(
                        "red-alert-command-single-target-error",
                        fluent_args![
                            "error" => error.to_string()
                        ],
                    )
                }
            }
            CommonRedAlertResult::Mass {
                results,
                auto_self_kick_result,
            } => {
                let mut actions_history = self.actions_history.lock().await;
                for result_index in 0..results.len() {
                    actions_history.log_history(
                        guild_id,
                        ActionType::TextRedAlert {
                            author_id,
                            target_id: target_users_ids[result_index],
                            is_success: results[result_index].is_deported(),
                        },
                    );
                }
                if let Some(auto_self_kick_result) = auto_self_kick_result {
                    actions_history.log_history(
                        guild_id,
                        ActionType::TextRedAlert {
                            author_id,
                            target_id: author_id,
                            is_success: auto_self_kick_result.is_deported(),
                        },
                    );
                    match auto_self_kick_result {
                        RedAlertDeportationResult::Deported => self
                            .l10n
                            .string("red-alert-command-mass-self-success", fluent_args![]),
                        RedAlertDeportationResult::NotFound => self
                            .l10n
                            .string("red-alert-command-mass-self-not-found", fluent_args![]),
                        RedAlertDeportationResult::Error(error) => self.l10n.string(
                            "red-alert-command-mass-self-error",
                            fluent_args![
                                "error" => error.to_string()
                            ],
                        ),
                    }
                } else {
                    let mut result_strings = vec![self
                        .l10n
                        .string("red-alert-command-mass-records-header", fluent_args![])];
                    for index in 0..results.len() {
                        let deport_status = match &results[index] {
                            RedAlertDeportationResult::Deported => self
                                .l10n
                                .string("red-alert-command-mass-success-status", fluent_args![]),
                            RedAlertDeportationResult::NotFound => self
                                .l10n
                                .string("red-alert-command-mass-not-found-status", fluent_args![]),
                            RedAlertDeportationResult::Error(error) => self.l10n.string(
                                "red-alert-command-mass-error-status",
                                fluent_args![
                                    "error" => error.to_string()
                                ],
                            ),
                        };
                        result_strings.push(self.l10n.string(
                            "red-alert-command-mass-record",
                            fluent_args![
                                "record-number" => index + 1,
                                "user-name" => target_users_ids[index].mention().to_string(),
                                "deport-status" => deport_status
                            ],
                        ))
                    }
                    result_strings.join(NEW_LINE)
                }
            }
        };
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}
