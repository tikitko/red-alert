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
    fn prefix_anchor(&self) -> &str {
        "код красный"
    }
    fn help_info<'a>(&'a self) -> Option<HelpInfo<'a>> {
        Some(HelpInfo {
            header_suffix: Some("{ID или упоминание пользователя}*"),
            description:
                "* - может быть несколько (через пробел).\nКикает выбранного пользователя из голосового канала если он в нем находится, иначе, кикает исполнителя команды.",
        })
    }
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
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
                    RedAlertDeportationResult::Deported => format!("ВИЖУ ТЫ ЗАБЫЛ УКАЗАТЬ ЦЕЛЬ ДЛЯ КРАСНОГО КОДА, НИЧЕГО... ШМАЛЬНЕМ В ТЕБЯ! (ИСПОЛЬЗУЙ ТЕГИ) ПРИНЯТО К ИСПОЛНЕНИЮ!"),
                    RedAlertDeportationResult::NotFound => format!(":face_with_monocle: ПОЛЬЗУЙСЯ ТЕГАМИ, И ЛУЧШЕ НЕ ЗАХОДИ В КАНАЛ, А ТО КИКНУ С ТАКИМИ ПРИКОЛАМИ! Пшшшш..."),
                    RedAlertDeportationResult::Error(_) => format!("СЛОМАЛСЯ ПОКА ПЫТАЛСЯ ТЕБЯ КИКНУТЬ ЧТО НЕПРАВИЛЬНОЕ ИСПОЛЬЗОВАНИЕ, КАК ВСЕГДА КОД ГОВНА! ОТМЕНА! Пшшшш...")
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
                    format!("КОД КРАСНЫЙ ПОДТВЕРЖДЕН! САМОВЫПИЛ ДЕЛО ДОСТОЙНОЕ!!! 0)00))00")
                } else {
                    let user_name = target_users_ids[0].mention();
                    format!("КОД КРАСНЫЙ ПОДТВЕРЖДЕН! АНТИКРИНЖ ОРУЖИЕ ИСПОЛЬЗОВАНО ПРОТИВ {user_name}!!! 0)00))00")
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
                        RedAlertDeportationResult::Deported => format!("В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЗНАЧИТ У ТЕБЯ БЕДЫ С БОШКОЙ, КОД КРАСНЫЙ НА ТЕБЯ!"),
                        RedAlertDeportationResult::NotFound => format!("ДОФИГА УМНЫЙ ВИЖУ? В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЖАЛЬ ТЕБЯ В КАНАЛЕ НЕТУ, ТАК БЫ ТЕБЯ ШМАЛЬНУЛ КОДОМ КРАСНЫМ! ОТМЕНА! Пшшшш..."),
                        RedAlertDeportationResult::Error(_) => format!("ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ТО ЧТО ТЫ ПЫТАЛСЯ КИКНУТЬ ТОГО КОГО НЕТ, НО Я СЛОМАЛСЯ! Пшшшш...")
                    }
                } else {
                    format!("СУИЦИД ЭТО ПЛОХО ТАК ЧТО НЕТ))) (У меня просто не получилось)")
                }
            }
            CommonRedAlertResult::SingleError {
                error: _,
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
                    format!("АУЧ, МАСЛИНУ ПОЙМАЛ, НЕ СМОГ ОРГАНИЗОВАТЬ ТЕБЕ СУИЦИД0))")
                } else {
                    format!("АУЧ, МАСЛИНУ ПОЙМАЛ, ОШИБКА В СИСТЕМЕё0))")
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
                        RedAlertDeportationResult::Deported => format!("МАССОВЫЙ КОД КРАСНЫЙ ШТУКА ОПАСНАЯ, ТАК КАК ПО РАЗНЫМ ПРИЧИНАМ Я НИКОГО НЕ КИКНУЛ, КИКНУ ТЕБЯ )В)В)))0"),
                        RedAlertDeportationResult::NotFound => format!("ЖАЛЬ ТЕБЯ НЕ МОГУ ПРШИТЬ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ! ОТМЕНА Пшшшш..."),
                        RedAlertDeportationResult::Error(_) => format!("ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ, НО САМ ОБО****СЯ! Пшшшш...")
                    }
                } else {
                    let mut result_strings = Vec::new();
                    for index in 0..results.len() {
                        let deport_status = &results[index];
                        let user_name = target_users_ids[index].mention();
                        let deport_status = match deport_status {
                            RedAlertDeportationResult::Deported => "ИСПОЛНЕНО",
                            RedAlertDeportationResult::NotFound => "НЕ В КАНАЛЕ",
                            RedAlertDeportationResult::Error(_) => "ОШИБКА (ПРОЧНЫЙ СУ*А)",
                        };
                        let record_number = index + 1;
                        result_strings.push(format!(
                            " {record_number}. {user_name} СТАТУС: {deport_status}."
                        ))
                    }
                    let result_string = result_strings.join("\n");
                    format!("ОУ, МАССОВЫЙ КОД КРАСНЫЙ? СТАТУС ВЫКОСА КРИНЖОВИКОВ:\n{result_string}")
                }
            }
        };
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}
