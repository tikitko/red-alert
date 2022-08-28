use crate::*;
use fon::chan::Ch16;
use fon::Audio;
use voskrust::api::{Model as VoskModel, Recognizer as VoskRecognizer};

#[derive(Debug, PartialEq, Clone)]
pub enum RecognitionResultType {
    Partial,
    Final,
}

#[derive(Debug, PartialEq, Clone)]
pub struct RecognitionResult {
    pub result_type: RecognitionResultType,
    pub text: String,
}

#[derive(Debug, PartialEq, Clone)]
pub enum RecognitionState {
    WaitingChunk,
    RepeatedResult,
    EmptyResult,
    Result(RecognitionResult),
}

pub struct Recognition<C: for<'a> VoiceContainer<'a>> {
    recognizer: VoskRecognizer,
    voice_container: C,
    last_partial: String,
    last_processed_chunk: usize,
}

impl<C: for<'a> VoiceContainer<'a>> Recognition<C> {
    pub const BASE_HZ: u32 = 16_000;
    pub fn new(voice_container: C, model: &VoskModel) -> Self {
        Self {
            recognizer: VoskRecognizer::new(model, Self::BASE_HZ as f32),
            voice_container,
            last_partial: "".to_string(),
            last_processed_chunk: 0,
        }
    }
}

impl<C: for<'a> VoiceContainer<'a>> Iterator for Recognition<C> {
    type Item = RecognitionState;
    fn next(&mut self) -> Option<Self::Item> {
        let voice = self.voice_container.blocking_voice();
        if voice.chunks.len() < (self.last_processed_chunk + 1) {
            if voice.is_completed {
                None
            } else {
                Some(RecognitionState::WaitingChunk)
            }
        } else {
            let audio_chunk = Audio::<Ch16, 2>::with_i16_buffer(
                48_000,
                voice.chunks[self.last_processed_chunk].as_slice(),
            );
            self.last_processed_chunk += 1;
            let mut simple_audio_chunk = Audio::<Ch16, 1>::with_audio(Self::BASE_HZ, &audio_chunk);
            if self
                .recognizer
                .accept_waveform(simple_audio_chunk.as_i16_slice())
            {
                let result = self.recognizer.final_result().to_string();
                if !result.is_empty() {
                    Some(RecognitionState::Result(RecognitionResult {
                        result_type: RecognitionResultType::Final,
                        text: result,
                    }))
                } else {
                    Some(RecognitionState::EmptyResult)
                }
            } else {
                let result = self.recognizer.partial_result().to_string();
                if result != self.last_partial {
                    self.last_partial = result.clone();
                    if !result.is_empty() {
                        Some(RecognitionState::Result(RecognitionResult {
                            result_type: RecognitionResultType::Partial,
                            text: result,
                        }))
                    } else {
                        Some(RecognitionState::EmptyResult)
                    }
                } else {
                    Some(RecognitionState::RepeatedResult)
                }
            }
        }
    }
}
