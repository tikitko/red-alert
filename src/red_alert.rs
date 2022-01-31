use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::*;

pub enum RedAlertDeportationUserResult {
    Deported,
    NotFound,
    Error(Error),
}

#[derive(Default)]
pub struct RedAlert;

impl RedAlert {
    pub async fn deportation(
        &self,
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
}
