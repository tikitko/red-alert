mod actions_history_command;
mod guilds_voice_config_command;
mod help_command_factory;
mod on_ready;
mod start_listen_command;
mod stop_listen_command;
mod text_command;

use actions_history_command::*;
use guilds_voice_config_command::*;
use help_command_factory::*;
use on_ready::*;
use start_listen_command::*;
use stop_listen_command::*;
use text_command::*;

use super::super::components::*;
use super::*;
use serenity::model::id::GuildId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use voskrust::api::Model as VoskModel;

pub struct RedAlertCommandsHandlerConstructor {
    pub recognition_model: VoskModel,
    pub listening_text: Option<String>,
    pub red_alert_handler: Arc<RedAlertHandler>,
    pub l10n: L10n,
}

impl RedAlertCommandsHandlerConstructor {
    pub fn build(self) -> Handler {
        let guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>> =
            Arc::new(Default::default());
        let actions_history: Arc<Mutex<RedAlertActionsHistory>> = Arc::new(Default::default());
        let guilds_voice_config = Arc::new(RwLock::new(RedAlertGuildsVoiceConfig::read()));
        Handler {
            help_command_factory: Box::new(RedAlertHelpCommandFactory {
                l10n: self.l10n.clone(),
            }),
            on_ready: Box::new(RedAlertOnReady {
                guilds_voices_receivers: guilds_voices_receivers.clone(),
                actions_history: actions_history.clone(),
                guilds_voice_config: guilds_voice_config.clone(),
                recognition_model: self.recognition_model,
                listening_text: self.listening_text,
                red_alert_handler: self.red_alert_handler.clone(),
                cancel_recognizer_sender: Arc::new(Mutex::new(None)),
                cancel_monitoring_sender: Arc::new(Mutex::new(None)),
            }),
            commands: vec![
                Box::new(TextRedAlertCommand {
                    red_alert_handler: self.red_alert_handler.clone(),
                    actions_history: actions_history.clone(),
                    l10n: self.l10n.clone(),
                }),
                Box::new(StartListenRedAlertCommand {
                    guilds_voices_receivers: guilds_voices_receivers.clone(),
                    l10n: self.l10n.clone(),
                }),
                Box::new(StopListenRedAlertCommand {
                    guilds_voices_receivers: guilds_voices_receivers.clone(),
                    l10n: self.l10n.clone(),
                }),
                Box::new(ActionsHistoryRedAlertCommand {
                    actions_history: actions_history.clone(),
                    l10n: self.l10n.clone(),
                }),
                Box::new(GuildsVoiceConfigRedAlertCommand {
                    guilds_voice_config: guilds_voice_config.clone(),
                    l10n: self.l10n.clone(),
                }),
            ],
        }
    }
}
