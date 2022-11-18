use crate::*;
use serenity::model::id::GuildId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct GuildsVoicesReceivers(pub Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>);

#[derive(Clone)]
pub struct GuildVoice {
    pub guild_id: GuildId,
    pub voice_container: ReceivingVoiceContainer,
}

impl GuildsVoicesReceivers {
    pub async fn next_guild_voice(&self) -> Option<GuildVoice> {
        let guilds_voices_receivers = self.0.read().await;
        for (guild_id, voice_receiver) in guilds_voices_receivers.iter() {
            let Some(voice_container) = voice_receiver.next_voice().await else {
                continue;
            };
            return Some(GuildVoice {
                guild_id: *guild_id,
                voice_container,
            });
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GuildIdInfo {
    pub guild_id: GuildId,
}

#[async_trait]
impl<'a> QueuedItemsContainer for GuildsVoicesReceivers {
    type Item = InfoVoiceContainer<GuildIdInfo, ReceivingVoiceContainer>;
    async fn next(&self) -> Option<Self::Item> {
        let Some(guild_voice) = self.next_guild_voice().await else {
            return None;
        };
        Some(InfoVoiceContainer {
            info: GuildIdInfo {
                guild_id: guild_voice.guild_id,
            },
            container: guild_voice.voice_container,
        })
    }
}
