use super::*;
use serenity::model::gateway::Activity;
use serenity::model::id::GuildId;
use serenity::model::prelude::{OnlineStatus, Ready, UserId};
use serenity::prelude::Context;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::oneshot::{channel, Sender};
use tokio::sync::{Mutex, RwLock};
use voskrust::api::Model as VoskModel;

pub(super) struct RedAlertOnReady {
    pub(super) guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
    pub(super) actions_history: Arc<Mutex<ActionsHistory>>,
    pub(super) guilds_voice_config: Arc<RwLock<RedAlertGuildsVoiceConfig>>,
    pub(super) recognition_model: VoskModel,
    pub(super) listening_text: Option<String>,
    pub(super) red_alert_handler: Arc<RedAlertHandler>,
    pub(super) cancel_sender: Arc<Mutex<Option<Sender<()>>>>,
}

impl RedAlertOnReady {
    async fn start_recognizer(&self, ctx: &Context) {
        let (tx, mut rx) = channel::<()>();
        let mut cancel_sender = self.cancel_sender.lock().await;
        *cancel_sender = Some(tx);
        drop(cancel_sender);
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
            let mut autors_processed_kicks: HashMap<UserId, HashSet<UserId>> = HashMap::new();
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
                        let mut kick_users_ids = guilds_voice_config
                            .get(&info.guild_id)
                            .should_kick(&info.user_id.0, &result.text)
                            .into_iter()
                            .map(|v| UserId(*v))
                            .collect::<HashSet<UserId>>();
                        drop(guilds_voice_config);
                        if let Some(mut autor_processed_kicks) =
                            autors_processed_kicks.remove(&info.user_id)
                        {
                            kick_users_ids = &kick_users_ids - &autor_processed_kicks;
                            autor_processed_kicks.extend(kick_users_ids.clone());
                            autors_processed_kicks.insert(info.user_id, autor_processed_kicks);
                        } else {
                            autors_processed_kicks.insert(info.user_id, kick_users_ids.clone());
                        }
                        if kick_users_ids.is_empty() {
                            continue;
                        };
                        info!(
                            "{} Recognition RESULT will be used for kick. Have restrictions.",
                            log_prefix
                        );
                        for kick_user_id in kick_users_ids {
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
                                    ActionType::VoiceRedAlert {
                                        author_id: info.user_id,
                                        target_id: kick_user_id,
                                        reason: result_text,
                                        is_success: deportation_result.is_deported(),
                                    },
                                );
                            });
                        }
                    }
                    RecognizerState::RecognitionStart(info) => {
                        info!("{} Recognition STARTED.", log_prefix);
                        autors_processed_kicks.remove(&info.user_id);
                    }
                    RecognizerState::RecognitionEnd(info) => {
                        info!("{} Recognition ENDED.", log_prefix);
                        autors_processed_kicks.remove(&info.user_id);
                    }
                }
            }
        });
    }
}

#[async_trait]
impl OnReady for RedAlertOnReady {
    async fn process(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
        let activity = self.listening_text.as_ref().map(|t| Activity::listening(t));
        ctx.set_presence(activity, OnlineStatus::Online).await;
        self.start_recognizer(&ctx).await;
    }
}
