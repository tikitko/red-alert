use serenity::model::prelude::UserId;
use std::collections::HashMap;

#[derive(Clone)]
pub struct VoiceConfig {
    pub target_words: Vec<String>,
    pub self_words: Vec<String>,
    pub aliases: HashMap<String, u64>,
}

impl VoiceConfig {
    pub fn should_kick(&self, author_user_id: UserId, text: &String) -> Option<UserId> {
        for self_word in &self.self_words {
            if !text.contains(self_word) {
                continue;
            }
            return Some(author_user_id);
        }
        for target_word in &self.target_words {
            if !text.contains(target_word) {
                continue;
            }
            for (name, user_id) in &self.aliases {
                if !text.contains(name) {
                    continue;
                }
                return Some(UserId(*user_id));
            }
        }
        None
    }
}
