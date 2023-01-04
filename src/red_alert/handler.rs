use serenity::model::prelude::*;
use serenity::prelude::*;

#[derive(Debug)]
pub enum RedAlertDeportationResult {
    Deported,
    NotFound,
    Error(SerenityError),
}

impl RedAlertDeportationResult {
    pub fn is_deported(&self) -> bool {
        match self {
            RedAlertDeportationResult::Deported => true,
            RedAlertDeportationResult::NotFound | RedAlertDeportationResult::Error(_) => false,
        }
    }
    pub fn is_not_found(&self) -> bool {
        match self {
            RedAlertDeportationResult::NotFound => true,
            RedAlertDeportationResult::Deported | RedAlertDeportationResult::Error(_) => false,
        }
    }
}

pub struct RedAlertHandler;

impl RedAlertHandler {
    pub async fn multiple(
        &self,
        ctx: &Context,
        guild_id: &GuildId,
        users_ids: &Vec<UserId>,
    ) -> Vec<RedAlertDeportationResult> {
        let mut results = Vec::with_capacity(users_ids.capacity());
        for index in 0..users_ids.len() {
            results.push(self.single(ctx, guild_id, &users_ids[index]).await);
        }
        results
    }

    pub async fn single(
        &self,
        ctx: &Context,
        guild_id: &GuildId,
        user_id: &UserId,
    ) -> RedAlertDeportationResult {
        if {
            if let Some(guild) = ctx.cache.guild(guild_id) {
                guild.voice_states.get(&user_id).is_some()
            } else {
                false
            }
        } {
            match guild_id.disconnect_member(&ctx, user_id).await {
                Ok(_) => RedAlertDeportationResult::Deported,
                Err(err) => RedAlertDeportationResult::Error(err),
            }
        } else {
            RedAlertDeportationResult::NotFound
        }
    }
}
