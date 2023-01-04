use crate::*;
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
pub struct RecognitionInfo<I: Copy> {
    pub user_id: UserId,
    pub inner: I,
}

#[derive(Debug, Clone)]
pub enum RecognizerState<I: Copy> {
    RecognitionStart(RecognitionInfo<I>),
    RecognitionResult(RecognitionInfo<I>, RecognitionResult),
    RecognitionEnd(RecognitionInfo<I>),
}

pub struct Recognizer<
    I: Copy + Send + Sync + Debug + 'static,
    C: for<'a> VoiceContainer<'a> + Send + Sync + 'static,
    Q: QueuedItemsContainer<Item = InfoVoiceContainer<I, C>> + Send + Sync + 'static,
> {
    pub model: VoskModel,
    pub voices_queue: Q,
}

impl<
        I: Copy + Send + Sync + Debug + 'static,
        C: for<'a> VoiceContainer<'a> + Send + Sync + 'static,
        Q: QueuedItemsContainer<Item = InfoVoiceContainer<I, C>> + Send + Sync + 'static,
    > Recognizer<I, C, Q>
{
    async fn recognition_task(
        sender: Sender<RecognizerState<I>>,
        info_voice_container: InfoVoiceContainer<I, C>,
        model: VoskModel,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let recognition_info = RecognitionInfo {
            user_id: *info_voice_container.container.user_id(),
            inner: info_voice_container.info,
        };
        sender
            .send(RecognizerState::RecognitionStart(recognition_info))
            .await?;
        let inner_sender = sender.clone();
        spawn_blocking(move || {
            let recognition = Recognition::new(info_voice_container.container, &model);
            for recognition_state in recognition {
                match recognition_state {
                    RecognitionState::RepeatedResult | RecognitionState::EmptyResult => {}
                    RecognitionState::WaitingChunk => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    RecognitionState::Result(recognition_result) => match inner_sender
                        .blocking_send(RecognizerState::RecognitionResult(
                            recognition_info,
                            recognition_result,
                        )) {
                        Ok(_) => {}
                        Err(error) => return Err(error),
                    },
                }
            }
            Ok(())
        })
        .await??;
        sender
            .send(RecognizerState::RecognitionEnd(recognition_info))
            .await?;
        Ok(())
    }
    pub fn start(self) -> Receiver<RecognizerState<I>> {
        let (tx, rx) = channel(1);
        spawn(async move {
            loop {
                if tx.is_closed() {
                    break;
                }
                sleep(Duration::from_millis(10)).await;
                let Some(info_voice_container) = self.voices_queue.next().await else {
                    continue;
                };
                spawn(Self::recognition_task(
                    tx.clone(),
                    info_voice_container,
                    self.model.clone(),
                ));
            }
        });
        rx
    }
}
