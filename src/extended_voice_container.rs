use crate::*;

#[derive(Clone)]
pub struct ExtendedVoiceContainer<I: Copy, C: for<'a> VoiceContainer<'a>> {
    pub information: I,
    pub container: C,
}
