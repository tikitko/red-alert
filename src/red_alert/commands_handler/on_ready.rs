use super::*;
use serenity::model::gateway::Activity;
use serenity::model::prelude::{OnlineStatus, Ready};
use serenity::prelude::Context;
use std::sync::Arc;
use tokio::sync::oneshot::Sender;
use tokio::sync::Mutex;

pub(super) struct RedAlertOnReady {
    pub(super) monitoring_performer: RedAlertMonitoringPerformer,
    pub(super) recognizer_performer: RedAlertRecognizerPerformer,
    pub(super) cancel_recognizer_sender: Arc<Mutex<Option<Sender<()>>>>,
    pub(super) cancel_monitoring_sender: Arc<Mutex<Option<Sender<()>>>>,
    pub(super) l10n: L10n,
}

#[async_trait]
impl OnReady for RedAlertOnReady {
    async fn process(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
        ctx.set_presence(
            Some(Activity::listening(
                self.l10n.string("listening-text", fluent_args![]),
            )),
            OnlineStatus::Online,
        )
        .await;

        let new_cancel_monitoring_sender = self.monitoring_performer.perform(&ctx);
        let mut cancel_monitoring_sender = self.cancel_monitoring_sender.lock().await;
        *cancel_monitoring_sender = Some(new_cancel_monitoring_sender);
        drop(cancel_monitoring_sender);

        let new_cancel_recognizer_sender = self.recognizer_performer.perform(&ctx);
        let mut cancel_recognizer_sender = self.cancel_recognizer_sender.lock().await;
        *cancel_recognizer_sender = Some(new_cancel_recognizer_sender);
        drop(cancel_recognizer_sender);
    }
}
