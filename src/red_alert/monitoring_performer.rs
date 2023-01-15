use super::super::components::*;
use super::*;
use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use serenity::prelude::Context;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot::{channel, Sender};
use tokio::sync::RwLock;

pub struct RedAlertMonitoringPerformer {
    pub guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
    pub guilds_voice_config: Arc<RwLock<RedAlertGuildsVoiceConfig>>,
}

impl RedAlertMonitoringPerformer {
    pub fn perform(&self, ctx: &Context) -> Sender<()> {
        let (tx, mut rx) = channel::<()>();
        let guilds_voices_receivers = self.guilds_voices_receivers.clone();
        let guilds_voice_config = self.guilds_voice_config.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            loop {
                let Some(_) = tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => Some(()),
                    _ = &mut rx => None,
                } else {
                    break;
                };
                let bot_user_id = ctx.cache.current_user_id();
                let guilds_voice_config = guilds_voice_config.read().await;
                for guild_id in guilds_voice_config.auto_track_ids() {
                    let Some(guild) = ctx.cache.guild(*guild_id) else {
                        continue;
                    };
                    let mut bot_channel_id = Option::<ChannelId>::None;
                    let mut channels_users_count = HashMap::<ChannelId, u8>::new();
                    for (user_id, voice_state) in guild.voice_states {
                        let Some(channel_id) = voice_state.channel_id else {
                            continue;
                        };
                        if bot_user_id == user_id {
                            bot_channel_id = Some(channel_id);
                            continue;
                        }
                        if voice_state.self_mute || voice_state.mute {
                            continue;
                        }
                        if let Some(users_count) = channels_users_count.remove(&channel_id) {
                            channels_users_count.insert(channel_id, users_count + 1);
                        } else {
                            channels_users_count.insert(channel_id, 1);
                        }
                    }
                    if let Some(channel_id) = {
                        let mut channels_users_count = channels_users_count
                            .into_iter()
                            .collect::<Vec<(ChannelId, u8)>>();
                        channels_users_count.sort_by(|a, b| {
                            let ordering = b.1.partial_cmp(&a.1).unwrap();
                            if ordering == Ordering::Equal {
                                b.0.partial_cmp(&a.0).unwrap()
                            } else {
                                ordering
                            }
                        });
                        channels_users_count.first().map(|c| c.0)
                    } {
                        if bot_channel_id.map_or_else(|| false, |i| i == channel_id) {
                            continue;
                        }
                        _ = start_listen(
                            guilds_voices_receivers.clone(),
                            &ctx,
                            guild.id,
                            channel_id,
                        )
                        .await;
                    } else {
                        if bot_channel_id.is_none() {
                            continue;
                        }
                        _ = stop_listen(guilds_voices_receivers.clone(), &ctx, guild.id).await;
                    }
                }
            }
        });
        tx
    }
}
