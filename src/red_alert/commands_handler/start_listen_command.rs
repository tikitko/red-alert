use super::*;
use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use serenity::model::prelude::Mention;
use serenity::prelude::{Context, Mentionable};
use std::collections::HashMap;
use std::ops::DerefMut;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(super) struct StartListenRedAlertCommand {
    pub(super) guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
    pub(super) l10n: L10n,
}

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

#[async_trait]
impl Command for StartListenRedAlertCommand {
    fn prefix_anchor(&self) -> String {
        self.l10n.string(
            "start-listen-red-alert-command-prefix-anchor",
            fluent_args![],
        )
    }
    fn help_info(&self) -> Option<HelpInfo> {
        Some(HelpInfo {
            header_suffix: Some(self.l10n.string(
                "start-listen-red-alert-command-header-suffix",
                fluent_args![],
            )),
            description: self.l10n.string(
                "start-listen-red-alert-command-help-description",
                fluent_args![],
            ),
        })
    }
    async fn process<'a>(&'a self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let channel_id: Option<ChannelId> = params
            .args
            .first()
            .map(|a| match Mention::from_str(a) {
                Ok(mention) => match mention {
                    Mention::Channel(channel_id) => Some(channel_id),
                    Mention::Role(_) | Mention::User(_) | Mention::Emoji(_, _) => None,
                },
                Err(_) => a.parse::<u64>().ok().map(ChannelId),
            })
            .flatten();
        let answer_msg = if let Some(channel_id) = channel_id {
            let channel_name = channel_id.mention().to_string();
            match start_listen(
                self.guilds_voices_receivers.clone(),
                &ctx,
                guild_id,
                channel_id,
            )
            .await
            {
                Ok(_) => self.l10n.string(
                    "start-listen-red-alert-command-success",
                    fluent_args![
                        "channel-name" => channel_name
                    ],
                ),
                Err(error) => match error {
                    StartListenError::ConnectingError => self.l10n.string(
                        "start-listen-red-alert-command-connect-error",
                        fluent_args![
                            "channel-name" => channel_name
                        ],
                    ),
                    StartListenError::SongbirdMissing => self.l10n.string(
                        "start-listen-red-alert-command-lib-error",
                        fluent_args![
                            "channel-name" => channel_name
                        ],
                    ),
                },
            }
        } else {
            self.l10n.string(
                "start-listen-red-alert-command-missed-channel",
                fluent_args![],
            )
        };
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}
