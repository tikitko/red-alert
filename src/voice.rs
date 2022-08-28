use async_trait::async_trait;
use serenity::model::prelude::UserId;
use std::ops::Deref;

pub struct Voice {
    pub id: u32,
    pub chunks: Vec<Vec<i16>>,
    pub is_completed: bool,
}

#[async_trait]
pub trait VoiceContainer<'a> {
    type Voice: Deref<Target = Voice> + 'a;
    fn user_id(&self) -> &UserId;
    async fn voice(&'a self) -> Self::Voice;
    fn blocking_voice(&'a self) -> Self::Voice;
}
