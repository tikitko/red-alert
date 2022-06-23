use serenity::model::prelude::*;
use serenity::prelude::*;

pub enum RedAlertDeportationUserResult {
    Deported,
    NotFound,
    Error(SerenityError),
}

pub enum RedAlertHandlerAnswer {
    Empty,
    NotGuildChat,
    BlockedUser(UserId),
    DeportationResult(std::collections::HashMap<UserId, RedAlertDeportationUserResult>),
}

pub struct RedAlertHandler {
    prefix_key_words: std::collections::HashSet<String>,
    users_ids_black_list: std::collections::HashSet<UserId>,
}

impl RedAlertHandler {
    pub fn new(
        prefix_key_words: std::collections::HashSet<String>,
        users_ids_black_list: std::collections::HashSet<UserId>,
    ) -> Self {
        Self {
            prefix_key_words,
            users_ids_black_list,
        }
    }

    fn is_contains_prefix_key_word(&self, content: &String) -> bool {
        let lowercase_content = content.to_lowercase();
        for prefix_key_word in &self.prefix_key_words {
            let lowercase_prefix_key_word = prefix_key_word.to_lowercase();
            if lowercase_content.strip_prefix(&lowercase_prefix_key_word) != None {
                return true;
            }
        }
        false
    }

    async fn deportation(
        ctx: &Context,
        guild: &Guild,
        execution_users_ids: std::collections::HashSet<UserId>,
    ) -> std::collections::HashMap<UserId, RedAlertDeportationUserResult> {
        let mut result = std::collections::HashMap::new();
        for user_id in execution_users_ids {
            if let Some(_voice_state) = guild.voice_states.get(&user_id) {
                match guild.id.disconnect_member(&ctx, user_id).await {
                    Ok(_member) => result.insert(user_id, RedAlertDeportationUserResult::Deported),
                    Err(err) => result.insert(user_id, RedAlertDeportationUserResult::Error(err)),
                };
            } else {
                result.insert(user_id, RedAlertDeportationUserResult::NotFound);
            }
        }
        return result;
    }

    pub async fn suicide_author_if_possible(
        &self,
        ctx: &Context,
        msg: &Message,
    ) -> Option<RedAlertDeportationUserResult> {
        let guild = if let Some(guild) = msg.guild(&ctx).await {
            guild
        } else {
            return None;
        };
        Self::deportation(
            &ctx,
            &guild,
            std::collections::HashSet::from([msg.author.id]),
        )
        .await
        .remove(&msg.author.id)
    }

    pub async fn answer(&self, ctx: &Context, msg: &Message) -> RedAlertHandlerAnswer {
        if msg.author.bot || !self.is_contains_prefix_key_word(&msg.content) {
            return RedAlertHandlerAnswer::Empty;
        }

        let guild = if let Some(guild) = msg.guild(&ctx).await {
            guild
        } else {
            return RedAlertHandlerAnswer::NotGuildChat;
        };

        if self.users_ids_black_list.contains(&msg.author.id) {
            return RedAlertHandlerAnswer::BlockedUser(msg.author.id);
        }

        let mentions = std::collections::HashSet::from_iter(msg.mentions.iter().map(|u| u.id));
        let deportation_result = Self::deportation(&ctx, &guild, mentions).await;

        RedAlertHandlerAnswer::DeportationResult(deportation_result)
    }
}
