use serenity::model::prelude::*;
use serenity::prelude::*;

pub enum RedAlertHandlerAnswer {
    Empty,
    Version(String),
    NotGuildChat,
    BlockedUser(UserId),
    DeportationResult(
        std::collections::HashMap<UserId, super::red_alert::RedAlertDeportationUserResult>,
    ),
}

pub struct RedAlertHandler {
    red_alert: super::red_alert::RedAlert,
    version: String,
    prefix_key_words: std::collections::HashSet<String>,
    users_ids_black_list: std::collections::HashSet<UserId>,
}

impl RedAlertHandler {
    pub fn new(
        version: String,
        prefix_key_words: std::collections::HashSet<String>,
        users_ids_black_list: std::collections::HashSet<UserId>,
    ) -> Self {
        Self {
            red_alert: Default::default(),
            version,
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

    pub async fn suicide_author_if_possible(
        &self,
        ctx: &Context,
        msg: &Message,
    ) -> Option<super::red_alert::RedAlertDeportationUserResult> {
        let guild = if let Some(guild) = msg.guild(&ctx).await {
            guild
        } else {
            return None;
        };
        self.red_alert
            .deportation(
                &ctx,
                &guild,
                std::collections::HashSet::from([msg.author.id]),
            )
            .await
            .remove(&msg.author.id)
    }

    pub async fn answer(&self, ctx: &Context, msg: &Message) -> RedAlertHandlerAnswer {
        if let (true, user) = (msg.mentions.len() == 1, ctx.cache.current_user().await) {
            if msg.mentions_user_id(user.id) {
                return RedAlertHandlerAnswer::Version(self.version.clone());
            }
        }

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
        let deportation_result = self.red_alert.deportation(&ctx, &guild, mentions).await;

        RedAlertHandlerAnswer::DeportationResult(deportation_result)
    }
}
