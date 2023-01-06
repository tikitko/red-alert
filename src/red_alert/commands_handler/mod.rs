mod actions_history_command;
mod guilds_voice_config_command;
mod help_command_factory;
mod on_ready;
mod start_listen_command;
mod stop_listen_command;
mod text_command;

use super::super::components::*;
use super::*;
use actions_history_command::*;
use chrono::{offset, DateTime, Utc};
use fluent::fluent_args;
use guilds_voice_config_command::*;
use help_command_factory::*;
use on_ready::*;
use serde::{Deserialize, Serialize};
use serenity::model::id::GuildId;
use serenity::model::prelude::UserId;
use start_listen_command::*;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use stop_listen_command::*;
use text_command::*;
use tokio::sync::{Mutex, RwLock};
use voskrust::api::Model as VoskModel;

pub struct RedAlertCommandsHandlerConstructor {
    pub recognition_model: VoskModel,
    pub listening_text: Option<String>,
    pub red_alert_handler: Arc<RedAlertHandler>,
    pub l10n: L10n,
}

enum ActionType {
    VoiceRedAlert {
        author_id: UserId,
        target_id: UserId,
        reason: String,
        is_success: bool,
    },
    TextRedAlert {
        author_id: UserId,
        target_id: UserId,
        is_success: bool,
    },
}

struct ActionInfo {
    time: DateTime<Utc>,
    r#type: ActionType,
}

#[derive(Default)]
struct ActionsHistory(HashMap<GuildId, VecDeque<ActionInfo>>);

impl ActionsHistory {
    fn log_history(&mut self, guild_id: GuildId, action_type: ActionType) {
        let action_info = ActionInfo {
            time: offset::Utc::now(),
            r#type: action_type,
        };
        if let Some(guild_actions_history) = self.0.get_mut(&guild_id) {
            guild_actions_history.push_back(action_info);
            if guild_actions_history.len() > 100 {
                guild_actions_history.pop_front();
            }
        } else {
            self.0.insert(guild_id, VecDeque::from([action_info]));
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct RedAlertGuildsVoiceConfig {
    base: RedAlertVoiceConfig<u64>,
    specific: HashMap<u64, RedAlertVoiceConfig<u64>>,
}

impl RedAlertGuildsVoiceConfig {
    const CONFIG_PATH: &str = "guilds_voice_config.yaml";
    fn read() -> RedAlertGuildsVoiceConfig {
        let config_string =
            std::fs::read_to_string(Self::CONFIG_PATH).expect("Guild voice config read error!");
        let config: RedAlertGuildsVoiceConfig =
            serde_yaml::from_str(&config_string).expect("Guild voice config deserialize error!");
        config
    }
    fn write(&self) {
        let config_string =
            serde_yaml::to_string(self).expect("Guild voice config serialize error!");
        std::fs::write(Self::CONFIG_PATH, config_string).expect("Guild voice config write error!");
    }
    fn get(&self, guild_id: &GuildId) -> &RedAlertVoiceConfig<u64> {
        self.specific.get(&guild_id.0).unwrap_or(&self.base)
    }
}

impl RedAlertCommandsHandlerConstructor {
    pub fn build(self) -> Handler {
        let guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>> =
            Arc::new(Default::default());
        let actions_history: Arc<Mutex<ActionsHistory>> = Arc::new(Default::default());
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
                cancel_sender: Arc::new(Mutex::new(None)),
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
                }),
                Box::new(GuildsVoiceConfigRedAlertCommand {
                    guilds_voice_config: guilds_voice_config.clone(),
                }),
            ],
        }
    }
}
