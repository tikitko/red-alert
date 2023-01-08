use super::*;
use serenity::model::id::GuildId;
use serenity::prelude::Context;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(super) struct StopListenRedAlertCommand {
    pub(super) guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
    pub(super) l10n: L10n,
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

#[async_trait]
impl Command for StopListenRedAlertCommand {
    fn prefix_anchor(&self) -> String {
        self.l10n.string(
            "stop-listen-red-alert-command-prefix-anchor",
            fluent_args![],
        )
    }
    fn help_info(&self) -> Option<HelpInfo> {
        Some(HelpInfo {
            header_suffix: None,
            description: self.l10n.string(
                "stop-listen-red-alert-command-help-description",
                fluent_args![],
            ),
        })
    }
    async fn process<'a>(&'a self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let answer_msg =
            match stop_listen(self.guilds_voices_receivers.clone(), &ctx, guild_id).await {
                Ok(_) => self
                    .l10n
                    .string("stop-listen-red-alert-command-success", fluent_args![]),
                Err(error) => match error {
                    StopListenError::DisconnectingError => self.l10n.string(
                        "stop-listen-red-alert-command-disconnect-error",
                        fluent_args![],
                    ),
                    StopListenError::SongbirdMissing => self
                        .l10n
                        .string("stop-listen-red-alert-command-lib-error", fluent_args![]),
                    StopListenError::NoListeners => self
                        .l10n
                        .string("stop-listen-red-alert-command-no-channel", fluent_args![]),
                },
            };
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}
