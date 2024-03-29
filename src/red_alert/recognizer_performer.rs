use super::super::components::*;
use super::*;
use serenity::model::id::GuildId;
use serenity::model::prelude::UserId;
use serenity::prelude::Context;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::oneshot::{channel, Sender};
use tokio::sync::{Mutex, RwLock};
use voskrust::api::Model as VoskModel;

pub struct RedAlertRecognizerPerformer {
    pub guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
    pub actions_history: Arc<Mutex<RedAlertActionsHistory>>,
    pub recognition_model: VoskModel,
    pub guilds_voice_config: Arc<RwLock<RedAlertGuildsVoiceConfig>>,
    pub red_alert_handler: Arc<RedAlertHandler>,
}

impl RedAlertRecognizerPerformer {
    pub fn perform(&self, ctx: &Context) -> Sender<()> {
        let (tx, mut rx) = channel::<()>();
        let guilds_voices_receivers = self.guilds_voices_receivers.clone();
        let actions_history = self.actions_history.clone();
        let recognition_model = self.recognition_model.clone();
        let guilds_voice_config = self.guilds_voice_config.clone();
        let red_alert_handler = self.red_alert_handler.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let mut recognizer_signal = Recognizer {
                model: recognition_model,
                voices_queue: GuildsVoicesReceivers(guilds_voices_receivers),
            }
            .start();
            let mut authors_processed_kicks: HashMap<UserId, HashSet<UserId>> = HashMap::new();
            loop {
                let Some(recognizer_state) = tokio::select! {
                    recognizer_state = recognizer_signal.recv() => recognizer_state,
                    _ = &mut rx => None,
                } else {
                    break;
                };
                let log_prefix = match recognizer_state {
                    RecognizerState::RecognitionStart(info)
                    | RecognizerState::RecognitionResult(info, _)
                    | RecognizerState::RecognitionEnd(info) => {
                        let mut prefix_parts: Vec<String> = vec![];
                        let guild_id = info.guild_id;
                        if let Some(guild) = ctx.cache.guild(guild_id) {
                            prefix_parts.push(format!("[G:{}]", guild.name));
                        } else {
                            prefix_parts.push(format!("[GID:{}]", guild_id));
                        }
                        let user_id = info.user_id;
                        if let Some(user) = ctx.cache.user(user_id) {
                            prefix_parts.push(format!(
                                "[U:{}#{:04}]",
                                user.name,
                                user.discriminator.min(9999).max(1)
                            ));
                        } else {
                            prefix_parts.push(format!("[UID:{}]", user_id));
                        }
                        prefix_parts.join("")
                    }
                };
                match recognizer_state {
                    RecognizerState::RecognitionResult(info, result) => {
                        info!(
                            "{} Recognition RESULT: type: {:?}, text: \"{}\".",
                            log_prefix, result.result_type, result.text
                        );
                        let guilds_voice_config = guilds_voice_config.read().await;
                        let users_ids_kicks_reasons = guilds_voice_config
                            .get(&info.guild_id)
                            .should_kick(&info.user_id.0, &result.text)
                            .into_iter()
                            .map(|v| (UserId(*v.0), v.1))
                            .collect::<HashMap<UserId, RedAlertVoiceSearchResult>>();
                        drop(guilds_voice_config);
                        let mut users_ids_kicks = users_ids_kicks_reasons
                            .keys()
                            .cloned()
                            .collect::<HashSet<UserId>>();
                        if let Some(mut author_processed_kicks) =
                            authors_processed_kicks.remove(&info.user_id)
                        {
                            users_ids_kicks = &users_ids_kicks - &author_processed_kicks;
                            author_processed_kicks.extend(users_ids_kicks.clone());
                            authors_processed_kicks.insert(info.user_id, author_processed_kicks);
                        } else {
                            authors_processed_kicks.insert(info.user_id, users_ids_kicks.clone());
                        }
                        if users_ids_kicks.is_empty() {
                            continue;
                        };
                        for (kick_user_id, kick_reason) in users_ids_kicks_reasons {
                            if !users_ids_kicks.contains(&kick_user_id) {
                                continue;
                            }
                            info!(
                                "{} Recognition RESULT will be used for kick. Have restriction \"{}\"({}) =~ \"{}\".",
                                log_prefix,
                                kick_reason.real_word,
                                kick_reason.total_similarity,
                                kick_reason.word
                            );
                            let actions_history = actions_history.clone();
                            let red_alert_handler = red_alert_handler.clone();
                            let ctx = ctx.clone();
                            let log_prefix = log_prefix.clone();
                            let result_text = result.text.clone();
                            tokio::spawn(async move {
                                let guild_id = info.guild_id;
                                let deportation_result = red_alert_handler
                                    .single(&ctx, &guild_id, &kick_user_id)
                                    .await;
                                info!(
                                    "{} Recognition RESULT used for kick, status is {:?}.",
                                    log_prefix, deportation_result
                                );
                                actions_history.lock().await.log_history(
                                    guild_id,
                                    RedAlertActionType::Voice {
                                        author_id: info.user_id,
                                        target_id: kick_user_id,
                                        full_text: result_text,
                                        reason: kick_reason,
                                        is_success: deportation_result.is_deported(),
                                    },
                                );
                            });
                        }
                    }
                    RecognizerState::RecognitionStart(info) => {
                        info!("{} Recognition STARTED.", log_prefix);
                        authors_processed_kicks.remove(&info.user_id);
                    }
                    RecognizerState::RecognitionEnd(info) => {
                        info!("{} Recognition ENDED.", log_prefix);
                        authors_processed_kicks.remove(&info.user_id);
                    }
                }
            }
        });
        tx
    }
}
