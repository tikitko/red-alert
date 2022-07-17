use crate::recognition::{Recognition, RecognitionResult, RecognitionState};
use crate::VoiceReceiver;
use serenity::model::id::UserId;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;
use voskrust::api::Model as VoskModel;

#[derive(Debug, PartialEq, Clone)]
pub enum RecognizerState {
    Idle,
    RecognitionStart(UserId),
    RecognitionResult(UserId, RecognitionResult),
    RecognitionEnd(UserId),
}

#[derive(Debug, PartialEq, Clone)]
pub struct RecognizerEvent {
    pub worker_number: usize,
    pub state: RecognizerState,
}

pub struct Recognizer {
    pub workers_count: usize,
    pub model: VoskModel,
    pub voice_receiver: VoiceReceiver,
}

impl Recognizer {
    pub fn start(self) -> Receiver<RecognizerEvent> {
        let (tx, rx) = mpsc::channel();
        for worker_index in 0..self.workers_count {
            let tx = tx.clone();
            let voice_receiver = self.voice_receiver.clone();
            let model = self.model.clone();
            thread::spawn(move || 'root: loop {
                thread::sleep(Duration::from_millis(500));
                let send_state = |state| -> bool {
                    tx.send(RecognizerEvent {
                        worker_number: worker_index + 1,
                        state,
                    })
                    .is_ok()
                };
                if let Some(voice) = voice_receiver.next_voice() {
                    let voice_user_id = voice.user_id();
                    if !send_state(RecognizerState::RecognitionStart(voice_user_id)) {
                        break 'root;
                    }
                    let recognition = Recognition::new(voice, &model);
                    for recognition_state in recognition {
                        match recognition_state {
                            RecognitionState::WaitingChunk
                            | RecognitionState::RepeatedResult
                            | RecognitionState::EmptyResult => {}
                            RecognitionState::Result(recognition_result) => {
                                if !send_state(RecognizerState::RecognitionResult(
                                    voice_user_id,
                                    recognition_result,
                                )) {
                                    break 'root;
                                }
                            }
                        }
                    }
                    if !send_state(RecognizerState::RecognitionEnd(voice_user_id)) {
                        break 'root;
                    }
                } else {
                    if !send_state(RecognizerState::Idle) {
                        break 'root;
                    }
                }
            });
        }
        rx
    }
}
