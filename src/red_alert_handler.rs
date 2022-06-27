use serenity::model::prelude::*;
use serenity::prelude::*;

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
        guild: &Guild,
        target_users_ids: Vec<UserId>,
    ) -> std::collections::HashMap<UserId, RedAlertDeportationResult> {
        let mut result = std::collections::HashMap::new();
        for user_id in target_users_ids {
            if let Some(_voice_state) = guild.voice_states.get(&user_id) {
                match guild.id.disconnect_member(&ctx, user_id).await {
                    Ok(_member) => result.insert(user_id, RedAlertDeportationResult::Deported),
                    Err(err) => result.insert(user_id, RedAlertDeportationResult::Error(err)),
                };
            } else {
                result.insert(user_id, RedAlertDeportationResult::NotFound);
            }
        }
        result
    }
}
