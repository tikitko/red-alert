use crate::VoiceReceiver;
use fon::chan::Ch16;
use fon::Audio;
use serenity::model::id::UserId;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;
use voskrust::api::{Model as VoskModel, Recognizer as VoskRecognizer};

#[derive(PartialEq, Clone)]
pub enum RecognitionResultType {
    Partial,
    Final,
}

#[derive(PartialEq, Clone)]
pub struct RecognitionResult {
    pub result_type: RecognitionResultType,
    pub text: String,
}

#[derive(PartialEq, Clone)]
pub struct RecognizerEvent {
    pub worker_number: usize,
    pub recognition_event: RecognitionEvent,
}

#[derive(PartialEq, Clone)]
pub enum RecognitionEvent {
    Idle,
    Start(Option<UserId>),
    Result(Option<UserId>, RecognitionResult),
    End(Option<UserId>),
}

pub struct Recognizer {
    pub workers_count: usize,
    pub model: VoskModel,
    pub voice_receiver: VoiceReceiver,
}

impl Recognizer {
    pub fn start(self) -> Receiver<RecognizerEvent> {
        let (tx, rx) = mpsc::channel();
        for worker_number in 1..self.workers_count {
            let tx = tx.clone();
            let voice_receiver = self.voice_receiver.clone();
            let model = self.model.clone();
            thread::spawn(move || 'root: loop {
                thread::sleep(Duration::from_millis(500));
                if let Some(voice) = voice_receiver.extract_voice() {
                    let voice_user_id = voice.user_id();
                    if tx
                        .send(RecognizerEvent {
                            worker_number,
                            recognition_event: RecognitionEvent::Start(voice_user_id),
                        })
                        .is_err()
                    {
                        break 'root;
                    }
                    let mut recognizer = VoskRecognizer::new(&model, 16_000 as f32);

                    let mut last_partial = String::new();
                    let mut last_processed_chunk: usize = 0;
                    loop {
                        let voice = voice.read_lock();
                        if voice.chunks.len() < (last_processed_chunk + 1) {
                            if voice.is_completed {
                                break;
                            } else {
                                continue;
                            }
                        } else {
                            let audio_chunk = Audio::<Ch16, 2>::with_i16_buffer(
                                48_000,
                                voice.chunks[last_processed_chunk].as_slice(),
                            );
                            let mut simple_audio_chunk =
                                Audio::<Ch16, 1>::with_audio(16_000, &audio_chunk);
                            if let Some(recognition_event) = {
                                if recognizer.accept_waveform(simple_audio_chunk.as_i16_slice()) {
                                    let result = recognizer.final_result();
                                    if !result.is_empty() {
                                        Some(RecognitionResult {
                                            result_type: RecognitionResultType::Final,
                                            text: result.to_string(),
                                        })
                                    } else {
                                        None
                                    }
                                } else {
                                    let result = recognizer.partial_result();
                                    if result != last_partial {
                                        last_partial = result.to_string();
                                        if !result.is_empty() {
                                            Some(RecognitionResult {
                                                result_type: RecognitionResultType::Partial,
                                                text: result.to_string(),
                                            })
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                }
                            } {
                                if tx
                                    .send(RecognizerEvent {
                                        worker_number,
                                        recognition_event: RecognitionEvent::Result(
                                            voice_user_id,
                                            recognition_event,
                                        ),
                                    })
                                    .is_err()
                                {
                                    break 'root;
                                }
                            }
                            last_processed_chunk += 1;
                        }
                    }
                    if tx
                        .send(RecognizerEvent {
                            worker_number,
                            recognition_event: RecognitionEvent::End(voice_user_id),
                        })
                        .is_err()
                    {
                        break 'root;
                    }
                } else {
                    if tx
                        .send(RecognizerEvent {
                            worker_number,
                            recognition_event: RecognitionEvent::Idle,
                        })
                        .is_err()
                    {
                        break 'root;
                    }
                }
            });
        }
        rx
    }
}
