use crate::*;
use guard::guard;
use serenity::model::id::UserId;
use std::error::Error;
use std::fmt::Debug;
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc::*;
use tokio::task::*;
use tokio::time::*;
use voskrust::api::Model as VoskModel;

#[derive(Debug, Clone, Copy)]
pub struct RecognitionInformation<I: Copy> {
    pub user_id: UserId,
    pub inner: I,
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

pub struct Recognizer<
    I: Copy + Send + Sync + Debug + 'static,
    C: for<'a> VoiceContainer<'a> + Send + Sync + 'static,
    Q: QueuedItemsContainer<Item = ExtendedVoiceContainer<I, C>> + Clone + Send + Sync + 'static,
> {
    pub workers_count: usize,
    pub model: VoskModel,
    pub voices_queue: Q,
}

impl<
        I: Copy + Send + Sync + Debug + 'static,
        C: for<'a> VoiceContainer<'a> + Send + Sync + 'static,
        Q: QueuedItemsContainer<Item = ExtendedVoiceContainer<I, C>> + Clone + Send + Sync + 'static,
    > Recognizer<I, C, Q>
{
    async fn worker_loop(
        sender: Sender<RecognizerState<I>>,
        voices_queue: Q,
        model: VoskModel,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            sleep(Duration::from_millis(100)).await;
            guard!(let Some(extended_voice_container) = voices_queue.next().await
            else {
                sender.send(RecognizerState::Idle).await?;
                continue;
            });
            let recognition_information = RecognitionInformation {
                user_id: *extended_voice_container.container.user_id(),
                inner: extended_voice_container.information,
            };
            sender
                .send(RecognizerState::RecognitionStart(recognition_information))
                .await?;
            let inner_sender = sender.clone();
            let model = model.clone();
            let inner_error = spawn_blocking(move || {
                let recognition = Recognition::new(extended_voice_container.container, &model);
                for recognition_state in recognition {
                    match recognition_state {
                        RecognitionState::RepeatedResult | RecognitionState::EmptyResult => {}
                        RecognitionState::WaitingChunk => {
                            thread::sleep(Duration::from_millis(1));
                        }
                        RecognitionState::Result(recognition_result) => match inner_sender
                            .blocking_send(RecognizerState::RecognitionResult(
                                recognition_information,
                                recognition_result,
                            )) {
                            Ok(_) => {}
                            Err(error) => return Some(error),
                        },
                    }
                }
                None
            })
            .await?;
            match inner_error {
                Some(inner_error) => return Err(inner_error.into()),
                None => {}
            }
            sender
                .send(RecognizerState::RecognitionEnd(recognition_information))
                .await?;
        }
    }
    pub fn start(self) -> Receiver<RecognizerEvent<I>> {
        let (tx, rx) = channel(100);
        for worker_index in 0..self.workers_count {
            let (ntx, mut nrx) = channel(tx.capacity());
            let voices_queue = self.voices_queue.clone();
            let model = self.model.clone();
            spawn(async move {
                let _ = Self::worker_loop(ntx, voices_queue, model).await;
            });
            let tx = tx.clone();
            spawn(async move {
                while let Some(recognizer_state) = nrx.recv().await {
                    if tx
                        .send(RecognizerEvent {
                            worker_number: worker_index + 1,
                            state: recognizer_state,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    };
                }
            });
        }
        rx
    }
}
