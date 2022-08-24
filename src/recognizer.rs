use crate::recognition::{Recognition, RecognitionResult, RecognitionState};
use crate::voices_storage::VoicesStorage;
use serenity::model::id::UserId;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;
use voskrust::api::Model as VoskModel;

#[derive(Debug, Clone, Copy)]
pub struct RecognitionInformation<I: Copy> {
    pub user_id: UserId,
    pub storage: I,
}

#[derive(Debug, Clone)]
pub enum RecognizerState<I: Copy> {
    Idle,
    RecognitionStart(RecognitionInformation<I>),
    RecognitionResult(RecognitionInformation<I>, RecognitionResult),
    RecognitionEnd(RecognitionInformation<I>),
}

#[derive(Debug, Clone)]
pub struct RecognizerEvent<I: Copy> {
    pub worker_number: usize,
    pub state: RecognizerState<I>,
}

pub struct Recognizer<S: VoicesStorage> {
    pub workers_count: usize,
    pub model: VoskModel,
    pub voices_storage: S,
}

impl<S: 'static + VoicesStorage> Recognizer<S> {
    pub fn start(self) -> Receiver<RecognizerEvent<S::Information>> {
        let (tx, rx) = mpsc::channel();
        for worker_index in 0..self.workers_count {
            let tx = tx.clone();
            let voices_storage = self.voices_storage.clone();
            let model = self.model.clone();
            thread::spawn(move || 'root: loop {
                thread::sleep(Duration::from_millis(100));
                let send_state = |state| -> bool {
                    tx.send(RecognizerEvent {
                        worker_number: worker_index + 1,
                        state,
                    })
                    .is_ok()
                };
                if let Some(storage_voice) = voices_storage.next_voice() {
                    let recognition_information = RecognitionInformation {
                        user_id: storage_voice.container.user_id(),
                        storage: storage_voice.information,
                    };
                    if !send_state(RecognizerState::RecognitionStart(recognition_information)) {
                        break 'root;
                    }
                    let recognition = Recognition::new(storage_voice.container, &model);
                    for recognition_state in recognition {
                        match recognition_state {
                            RecognitionState::RepeatedResult | RecognitionState::EmptyResult => {}
                            RecognitionState::WaitingChunk => {
                                thread::sleep(Duration::from_millis(1));
                            }
                            RecognitionState::Result(recognition_result) => {
                                if !send_state(RecognizerState::RecognitionResult(
                                    recognition_information,
                                    recognition_result,
                                )) {
                                    break 'root;
                                }
                            }
                        }
                    }
                    if !send_state(RecognizerState::RecognitionEnd(recognition_information)) {
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
