use super::*;
use serde::{Deserialize, Serialize};
use serenity::model::id::GuildId;
use std::collections::{HashMap, HashSet};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct RedAlertGuildsVoiceConfig {
    auto_track_ids: HashSet<u64>,
    base: RedAlertVoiceConfig<u64>,
    specific: HashMap<u64, RedAlertVoiceConfig<u64>>,
}

impl RedAlertGuildsVoiceConfig {
    const CONFIG_PATH: &str = "guilds_voice_config.yaml";
    pub fn read() -> RedAlertGuildsVoiceConfig {
        let config_string =
            std::fs::read_to_string(Self::CONFIG_PATH).expect("Guild voice config read error!");
        let config: RedAlertGuildsVoiceConfig =
            serde_yaml::from_str(&config_string).expect("Guild voice config deserialize error!");
        config
    }
    pub fn write(&self) {
        let config_string =
            serde_yaml::to_string(self).expect("Guild voice config serialize error!");
        std::fs::write(Self::CONFIG_PATH, config_string).expect("Guild voice config write error!");
    }
    pub fn get(&self, guild_id: &GuildId) -> &RedAlertVoiceConfig<u64> {
        self.specific.get(&guild_id.0).unwrap_or(&self.base)
    }
    pub fn remove(&mut self, guild_id: &GuildId) -> RedAlertVoiceConfig<u64> {
        self.specific
            .remove(&guild_id.0)
            .unwrap_or(self.base.clone())
    }
    pub fn insert(&mut self, guild_id: GuildId, guild_voice_config: RedAlertVoiceConfig<u64>) {
        self.specific.insert(guild_id.0, guild_voice_config);
    }
    pub fn switch_auto_track(&mut self, guild_id: GuildId) -> bool {
        if self.auto_track_ids.remove(&guild_id.0) {
            false
        } else {
            self.auto_track_ids.insert(guild_id.0);
            true
        }
    }
    pub fn auto_track_ids(&self) -> &HashSet<u64> {
        &self.auto_track_ids
    }
}
