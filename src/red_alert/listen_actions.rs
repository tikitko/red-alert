use super::super::components::*;
use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use serenity::prelude::Context;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use tokio::sync::RwLock;

pub enum StartListenError {
    SongbirdMissing,
    ConnectingError,
}

pub async fn start_listen(
    guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<(), StartListenError> {
    let Some(manager) = songbird::get(ctx).await else {
        return Err(StartListenError::SongbirdMissing);
    };
    let (handler_lock, connection_result) = manager.join(guild_id, channel_id).await;
    if !connection_result.is_ok() {
        return Err(StartListenError::ConnectingError);
    }
    let mut handler = handler_lock.lock().await;
    _ = handler.mute(true).await;
    let voice_receiver = VoiceReceiver::with_configuration(Default::default());
    voice_receiver.subscribe(handler.deref_mut());
    let mut guilds_voices_receivers = guilds_voices_receivers.write().await;
    guilds_voices_receivers.insert(guild_id, voice_receiver);
    Ok(())
}

pub enum StopListenError {
    SongbirdMissing,
    DisconnectingError,
    NoListeners,
}

pub async fn stop_listen(
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
