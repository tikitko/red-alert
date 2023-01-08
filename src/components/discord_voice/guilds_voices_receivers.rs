use super::super::base::*;
use super::super::voice::*;
use super::*;
use serenity::model::id::GuildId;
use serenity::model::prelude::UserId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct GuildsVoicesReceivers(pub Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>);

#[derive(Debug, Clone, Copy)]
pub struct GuildVoiceInfo {
    pub user_id: UserId,
    pub guild_id: GuildId,
}

impl GuildsVoicesReceivers {
    pub async fn next_guild_voice(
        &self,
    ) -> Option<InfoVoiceContainer<GuildVoiceInfo, ReceivingVoiceContainer>> {
        let guilds_voices_receivers = self.0.read().await;
        for (guild_id, voice_receiver) in guilds_voices_receivers.iter() {
            let Some(voice_container) = voice_receiver.next_voice().await else {
                continue;
            };
            return Some(InfoVoiceContainer {
                info: GuildVoiceInfo {
                    user_id: voice_container.info.client_user_id,
                    guild_id: *guild_id,
                },
                container: voice_container.container,
            });
        }
        None
    }
}

#[async_trait]
impl<'a> QueuedItemsContainer for GuildsVoicesReceivers {
    type Item = InfoVoiceContainer<GuildVoiceInfo, ReceivingVoiceContainer>;
    async fn next(&self) -> Option<Self::Item> {
        self.next_guild_voice().await
    }
}
