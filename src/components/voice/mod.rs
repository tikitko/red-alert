mod recognition;
mod recognizer;

pub use recognition::*;
pub use recognizer::*;

use std::ops::Deref;

pub struct Voice {
    pub id: u32,
    pub chunks: Vec<Vec<i16>>,
    pub is_completed: bool,
}

#[async_trait]
pub trait VoiceContainer<'a> {
    type Voice: Deref<Target = Voice> + 'a;
    async fn voice(&'a self) -> Self::Voice;
    fn blocking_voice(&'a self) -> Self::Voice;
}

pub struct InfoVoiceContainer<I, C: for<'a> VoiceContainer<'a>> {
    pub info: I,
    pub container: C,
}
