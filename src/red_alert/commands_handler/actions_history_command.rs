use super::*;
use serenity::prelude::{Context, Mentionable};
use std::sync::Arc;

pub(super) struct ActionsHistoryRedAlertCommand {
    pub(super) actions_history: Arc<Mutex<ActionsHistory>>,
}

#[async_trait]
impl Command for ActionsHistoryRedAlertCommand {
    fn prefix_anchor(&self) -> &str {
        "код красный история"
    }
    fn help_info<'a>(&'a self) -> Option<HelpInfo<'a>> {
        Some(HelpInfo {
            header_suffix: None,
            description: "Выводит историю всех наказаний которые исполнил КРИНЖ КИЛЛЕР.",
        })
    }
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let mut actions_history = self.actions_history.lock().await;
        let answer_msg = if let Some(guild_history) = (*actions_history).0.get(&guild_id) {
            let mut result_strings = vec![];
            for action_info_index in 0..guild_history.len() {
                let time = &guild_history[action_info_index].time.to_rfc2822();
                let info_string = match &guild_history[action_info_index].r#type {
                    ActionType::VoiceRedAlert {
                        author_id,
                        target_id,
                        reason,
                        is_success,
                    } => {
                        let autor = author_id.mention();
                        let target = target_id.mention();
                        if author_id == target_id {
                            let status = if *is_success {
                                "САМОВЫПИЛИЛСЯ"
                            } else {
                                "ПОПЫТАЛСЯ САМОВЫПИЛИТЬСЯ"
                            };
                            format!("КРИНЖОВИК {target} {status} ФРАЗОЙ \"{reason}\".")
                        } else {
                            let status = if *is_success {
                                "КИКНУТ"
                            } else {
                                "ПОЧТИ... КИКНУТ"
                            };
                            format!("КРИНЖОВИК {target} {status} ГОЛОСОМ МИРОТВОРЦA {autor} ПРИ ПОМОЩИ ФРАЗЫ \"{reason}\".")
                        }
                    }
                    ActionType::TextRedAlert {
                        author_id,
                        target_id,
                        is_success,
                    } => {
                        let author = author_id.mention();
                        let target = target_id.mention();
                        if author_id == target_id {
                            let status = if *is_success {
                                "САМОВЫПИЛИЛСЯ"
                            } else {
                                "ПОПЫТАЛСЯ САМОВЫПИЛИТЬСЯ"
                            };
                            format!("КРИНЖОВИК {target} {status} КОМАНДОЙ")
                        } else {
                            let status = if *is_success {
                                "КИКНУТ"
                            } else {
                                "ПОЧТИ... КИКНУТ"
                            };
                            format!("КРИНЖОВИК {target} {status} КОМАНДОЙ МИРОТВОРЦA {author}")
                        }
                    }
                };
                let record_number = action_info_index + 1;
                result_strings.push(format!(" {record_number}. [ВРЕМЯ: {time}] {info_string}."));
            }
            let result_string = result_strings.join("\n");
            format!("ИСТОРИЯ ВЫКОСА КРИНЖОВИКОВ:\n{result_string}")
        } else {
            format!("ПОКА ЕЩЕ НИКОГО НЕ УШАТАЛ НА ЭТОМ СЕРВЕР)!1!))")
        };
        (*actions_history).0.remove(&guild_id);
        drop(actions_history);
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}
