use serenity::model::prelude::*;
use serenity::prelude::*;

#[derive(Debug)]
pub enum RedAlertDeportationResult {
    Deported,
    NotFound,
    Error(SerenityError),
}

pub struct RedAlertHandler;

impl RedAlertHandler {
    pub async fn handle<'a>(
        &self,
        ctx: &Context,
        guild_id: GuildId,
        target_users_ids: Vec<UserId>,
    ) -> std::collections::HashMap<UserId, RedAlertDeportationResult> {
        let guild = ctx.cache.guild(guild_id).await;
        let mut result = std::collections::HashMap::new();
        for user_id in target_users_ids {
            if {
                if let Some(guild) = &guild {
                    guild.voice_states.get(&user_id).is_some()
                } else {
                    false
                }
            } {
                match guild_id.disconnect_member(&ctx, user_id).await {
                    Ok(_) => result.insert(user_id, RedAlertDeportationResult::Deported),
                    Err(err) => result.insert(user_id, RedAlertDeportationResult::Error(err)),
                };
            } else {
                result.insert(user_id, RedAlertDeportationResult::NotFound);
            }
        }
        result
    }
}
