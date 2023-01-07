use super::*;
use serenity::cache;
use serenity::model::gateway::Activity;
use serenity::model::id::GuildId;
use serenity::model::prelude::{ChannelId, OnlineStatus, Ready, UserId};
use serenity::prelude::Context;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot::{channel, Sender};
use tokio::sync::{Mutex, RwLock};
use tokio::time::*;
use voskrust::api::Model as VoskModel;

pub(super) struct RedAlertOnReady {
    pub(super) guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
    pub(super) actions_history: Arc<Mutex<ActionsHistory>>,
    pub(super) guilds_voice_config: Arc<RwLock<RedAlertGuildsVoiceConfig>>,
    pub(super) recognition_model: VoskModel,
    pub(super) listening_text: Option<String>,
    pub(super) red_alert_handler: Arc<RedAlertHandler>,
    pub(super) cancel_recognizer_sender: Arc<Mutex<Option<Sender<()>>>>,
    pub(super) cancel_monitoring_sender: Arc<Mutex<Option<Sender<()>>>>,
}

impl RedAlertOnReady {
    async fn start_recognizer(&self, ctx: &Context) {
        let (tx, mut rx) = channel::<()>();
        let mut cancel_sender = self.cancel_recognizer_sender.lock().await;
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
                                    ActionType::VoiceRedAlert {
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
    }
    async fn start_monitoring(&self, ctx: &Context) {
        let (tx, mut rx) = channel::<()>();
        let mut cancel_sender = self.cancel_monitoring_sender.lock().await;
        *cancel_sender = Some(tx);
        drop(cancel_sender);
        let guilds_voices_receivers = self.guilds_voices_receivers.clone();
        let guilds_voice_config = self.guilds_voice_config.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let mut guilds_active_channels: HashMap<GuildId, ChannelId> = HashMap::new();
            loop {
                let Some(_) = tokio::select! {
                    _ = sleep(Duration::from_secs(1)) => Some(()),
                    _ = &mut rx => None,
                } else {
                    break;
                };
                let bot_user_id = ctx.cache.current_user_id();
                let guilds_voice_config = guilds_voice_config.read().await;
                for guild_id in &guilds_voice_config.auto_track_guilds_ids {
                    let Some(guild) = ctx.cache.guild(*guild_id) else {
                        continue;
                    };
                    let mut channels_users_counts: HashMap<ChannelId, u8> = HashMap::new();
                    for (user_id, voice_state) in guild.voice_states {
                        if bot_user_id == user_id {
                            continue;
                        }
                        let Some(channel_id) = voice_state.channel_id else {
                            continue;
                        };
                        if let Some(users_count) = channels_users_counts.remove(&channel_id) {
                            channels_users_counts.insert(channel_id, users_count + 1);
                        } else {
                            channels_users_counts.insert(channel_id, 1);
                        }
                    }
                    if let Some(channel_id) = {
                        let mut channels_users_counts = channels_users_counts
                            .into_iter()
                            .collect::<Vec<(ChannelId, u8)>>();
                        channels_users_counts.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                        channels_users_counts.first().map(|c| c.0)
                    } {
                        let is_prev_channel = guilds_active_channels
                            .get(&guild.id)
                            .map_or_else(|| false, |i| i == &channel_id);
                        if is_prev_channel {
                            continue;
                        }
                        guilds_active_channels.insert(guild.id, channel_id);
                        _ = start_listen(
                            guilds_voices_receivers.clone(),
                            &ctx,
                            guild.id,
                            channel_id,
                        )
                        .await;
                    } else {
                        if !guilds_active_channels.remove(&guild.id).is_some() {
                            continue;
                        }
                        _ = stop_listen(guilds_voices_receivers.clone(), &ctx, guild.id).await;
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
        self.start_monitoring(&ctx).await;
    }
}
