use super::*;
use chrono::{offset, DateTime, Utc};
use serenity::model::id::GuildId;
use serenity::model::prelude::UserId;
use std::collections::{HashMap, VecDeque};

pub enum RedAlertActionType {
    Voice {
        author_id: UserId,
        target_id: UserId,
        full_text: String,
        reason: RedAlertVoiceSearchResult,
        is_success: bool,
    },
    Text {
        author_id: UserId,
        target_id: UserId,
        is_success: bool,
    },
}

pub struct RedAlertActionInfo {
    pub time: DateTime<Utc>,
    pub r#type: RedAlertActionType,
}

#[derive(Default)]
pub struct RedAlertActionsHistory(HashMap<GuildId, VecDeque<RedAlertActionInfo>>);

impl RedAlertActionsHistory {
    pub fn log_history(&mut self, guild_id: GuildId, action_type: RedAlertActionType) {
        let action_info = RedAlertActionInfo {
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
    pub fn extract(&mut self, guild_id: &GuildId) -> Option<VecDeque<RedAlertActionInfo>> {
        self.0.remove(guild_id)
    }
}
