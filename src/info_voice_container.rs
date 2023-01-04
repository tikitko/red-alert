use crate::*;

pub struct InfoVoiceContainer<I, C: for<'a> VoiceContainer<'a>> {
    pub info: I,
    pub container: C,
}
