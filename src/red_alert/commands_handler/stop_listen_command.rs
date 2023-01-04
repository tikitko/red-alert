use super::*;
use serenity::model::id::GuildId;
use serenity::prelude::Context;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(super) struct StopListenRedAlertCommand {
    pub(super) guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
}

enum StopListenError {
    SongbirdMissing,
    DisconnectingError,
    NoListeners,
}

async fn stop_listen(
    guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
    ctx: &Context,
    guild_id: GuildId,
) -> Result<(), StopListenError> {
    let Some(manager) = songbird::get(ctx).await else {
        return Err(StopListenError::SongbirdMissing);
    };
    if !manager.get(guild_id).is_some() {
        return Err(StopListenError::NoListeners);
    }
    if manager.remove(guild_id).await.is_err() {
        return Err(StopListenError::DisconnectingError);
    }
    let mut guilds_voices_receivers = guilds_voices_receivers.write().await;
    guilds_voices_receivers.remove(&guild_id);
    Ok(())
}

#[async_trait]
impl Command for StopListenRedAlertCommand {
    fn prefix_anchor(&self) -> &str {
        "прекратить слушать код красный"
    }
    fn help_info<'a>(&'a self) -> Option<HelpInfo<'a>> {
        Some(HelpInfo {
            header_suffix: None,
            description:
                "Прекратить слушать голосовой канал в котором находится КРИНЖ КИЛЛЕР на запрещенные и направленные фразы.",
        })
    }
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let answer_msg =
            match stop_listen(self.guilds_voices_receivers.clone(), &ctx, guild_id).await {
                Ok(_) => {
                    format!("ПРЕКРАЩАЮ ОТСЛЕЖИВАНИЕ КАНАЛА!")
                }
                Err(error) => match error {
                    StopListenError::DisconnectingError => {
                        format!("ПРОИЗОШЛА ОШИБКА! НЕ ПОЛУЧАЕТСЯ ОТКЛЮЧИТЬСЯ...")
                    }
                    StopListenError::SongbirdMissing => {
                        format!("ЗВУКОВАЯ БИБЛИОТЕКА ОТСУТСТВУЕТ...")
                    }
                    StopListenError::NoListeners => format!("НЕ ОТСЛЕЖИВАЮ КАНАЛЫ!"),
                },
            };
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}
