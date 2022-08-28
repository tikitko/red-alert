use crate::*;

#[derive(Clone)]
pub struct InfoVoiceContainer<I: Copy, C: for<'a> VoiceContainer<'a>> {
    pub info: I,
    pub container: C,
}
