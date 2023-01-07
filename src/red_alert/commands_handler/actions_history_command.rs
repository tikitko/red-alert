use super::*;
use serenity::prelude::{Context, Mentionable};
use std::sync::Arc;

pub(super) struct ActionsHistoryRedAlertCommand {
    pub(super) actions_history: Arc<Mutex<ActionsHistory>>,
    pub(super) l10n: L10n,
}

#[async_trait]
impl Command for ActionsHistoryRedAlertCommand {
    fn prefix_anchor(&self) -> String {
        self.l10n.string(
            "actions-history-red-alert-command-prefix-anchor",
            fluent_args![],
        )
    }
    fn help_info(&self) -> Option<HelpInfo> {
        Some(HelpInfo {
            header_suffix: None,
            description: self.l10n.string(
                "actions-history-red-alert-command-help-description",
                fluent_args![],
            ),
        })
    }
    async fn process<'a>(&'a self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let mut actions_history = self.actions_history.lock().await;
        let answer_msg = if let Some(guild_history) = (*actions_history).0.get(&guild_id) {
            let mut result_strings = vec![self.l10n.string(
                "actions-history-red-alert-command-list-header",
                fluent_args![],
            )];
            for action_info_index in 0..guild_history.len() {
                let info_string = match &guild_history[action_info_index].r#type {
                    ActionType::VoiceRedAlert {
                        author_id,
                        target_id,
                        full_text,
                        reason,
                        is_success,
                    } => {
                        let reason_text = full_text.replace(
                            reason.real_word.as_str(),
                            self.l10n
                                .string(
                                    "actions-history-red-alert-command-voice-record-reason-format",
                                    fluent_args![
                                        "reason" => reason.real_word.to_owned()
                                    ],
                                )
                                .as_str(),
                        );
                        let target_name = target_id.mention().to_string();
                        if author_id == target_id {
                            self.l10n.string(
                                "actions-history-red-alert-command-voice-self-record",
                                fluent_args![
                                    "target-name" => target_name,
                                    "status" => if *is_success {
                                        self.l10n.string(
                                            "actions-history-red-alert-command-self-kick-status-success",
                                            fluent_args![],
                                        )
                                    } else {
                                        self.l10n.string(
                                            "actions-history-red-alert-command-self-kick-status-fail",
                                            fluent_args![],
                                        )
                                    },
                                    "reason-text" => reason_text,
                                    "restricted-word" => reason.word.to_owned(),
                                    "similarity-percent" => (reason.total_similarity * 100.0) as u8
                                ],
                            )
                        } else {
                            self.l10n.string(
                                "actions-history-red-alert-command-voice-target-record",
                                fluent_args![
                                    "target-name" => target_name,
                                    "status" => if *is_success {
                                        self.l10n.string(
                                            "actions-history-red-alert-command-target-kick-status-success",
                                            fluent_args![],
                                        )
                                    } else {
                                        self.l10n.string(
                                            "actions-history-red-alert-command-target-kick-status-fail",
                                            fluent_args![],
                                        )
                                    },
                                    "author-name" => author_id.mention().to_string(),
                                    "reason-text" => reason_text,
                                    "restricted-word" => reason.word.to_owned(),
                                    "similarity-percent" => (reason.total_similarity * 100.0) as u8
                                ],
                            )
                        }
                    }
                    ActionType::TextRedAlert {
                        author_id,
                        target_id,
                        is_success,
                    } => {
                        let target_name = target_id.mention().to_string();
                        if author_id == target_id {
                            self.l10n.string(
                                "actions-history-red-alert-command-text-self-record",
                                fluent_args![
                                    "target-name" => target_name,
                                    "status" => if *is_success {
                                        self.l10n.string(
                                            "actions-history-red-alert-command-self-kick-status-success",
                                            fluent_args![],
                                        )
                                    } else {
                                        self.l10n.string(
                                            "actions-history-red-alert-command-self-kick-status-fail",
                                            fluent_args![],
                                        )
                                    }
                                ],
                            )
                        } else {
                            self.l10n.string(
                                "actions-history-red-alert-command-text-target-record",
                                fluent_args![
                                    "target-name" => target_name,
                                    "status" => if *is_success {
                                        self.l10n.string(
                                            "actions-history-red-alert-command-target-kick-status-success",
                                            fluent_args![],
                                        )
                                    } else {
                                        self.l10n.string(
                                            "actions-history-red-alert-command-target-kick-status-fail",
                                            fluent_args![],
                                        )
                                    },
                                    "author-name" => author_id.mention().to_string()
                                ],
                            )
                        }
                    }
                };
                result_strings.push(self.l10n.string(
                    "actions-history-red-alert-command-record",
                    fluent_args![
                        "record-number" => action_info_index + 1,
                        "time" => guild_history[action_info_index].time.format(self.l10n.string(
                            "actions-history-red-alert-command-voice-record-time-format",
                            fluent_args![],
                        ).as_str()).to_string(),
                        "record" => info_string
                    ],
                ));
            }
            result_strings.join(NEW_LINE)
        } else {
            self.l10n.string(
                "actions-history-red-alert-command-empty-list",
                fluent_args![],
            )
        };
        (*actions_history).0.remove(&guild_id);
        drop(actions_history);
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}
