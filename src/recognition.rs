use crate::VoiceReceiver;
use serenity::model::id::UserId;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;
use voskrust::api::{Model, Recognizer};

pub enum RecognitionWorkerEvent {
    Idle(usize),
    Start(usize, Option<UserId>),
    Event(usize, Option<UserId>, RecognitionEvent),
    End(usize, Option<UserId>),
}

#[derive(PartialEq, Clone)]
pub enum RecognitionEventType {
    Partial,
    Result,
}

#[derive(PartialEq, Clone)]
pub struct RecognitionEvent {
    pub text_type: RecognitionEventType,
    pub text: String,
}

pub fn start_recognition(
    workers_count: usize,
    model: Model,
    voice_receiver: VoiceReceiver,
) -> Receiver<RecognitionWorkerEvent> {
    let (tx, rx) = mpsc::channel();
    for worker_number in 1..workers_count {
        let tx = tx.clone();
        let voice_receiver = voice_receiver.clone();
        let model = model.clone();
        thread::spawn(move || 'root: loop {
            thread::sleep(Duration::from_millis(500));
            if let Some(voice) = voice_receiver.extract_voice() {
                let voice_user_id = voice.user_id();
                if tx
                    .send(RecognitionWorkerEvent::Start(worker_number, voice_user_id))
                    .is_err()
                {
                    break 'root;
                }
                let mut recognizer = Recognizer::new(&model, voice_receiver.output_hz() as f32);

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
                        let voice_chunk = &voice.chunks[last_processed_chunk];
                        let completed = recognizer.accept_waveform(voice_chunk);
                        if let Some(recognition_event) = {
                            if completed {
                                let result = recognizer.final_result();
                                if !result.is_empty() {
                                    Some(RecognitionEvent {
                                        text_type: RecognitionEventType::Result,
                                        text: result.to_string(),
                                    })
                                } else {
                                    None
                                }
                            } else {
                                let result = recognizer.partial_result();
                                if result != last_partial {
                                    last_partial.clear();
                                    last_partial.insert_str(0, &result);
                                    if !result.is_empty() {
                                        Some(RecognitionEvent {
                                            text_type: RecognitionEventType::Partial,
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
                                .send(RecognitionWorkerEvent::Event(
                                    worker_number,
                                    voice_user_id,
                                    recognition_event,
                                ))
                                .is_err()
                            {
                                break 'root;
                            }
                        }
                        last_processed_chunk += 1;
                    }
                }
                if tx
                    .send(RecognitionWorkerEvent::End(worker_number, voice_user_id))
                    .is_err()
                {
                    break 'root;
                }
            } else {
                if tx
                    .send(RecognitionWorkerEvent::Idle(worker_number))
                    .is_err()
                {
                    break 'root;
                }
            }
        });
    }
    rx
}
