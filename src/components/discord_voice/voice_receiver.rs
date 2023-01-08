use super::super::base::*;
use super::super::voice::*;
use bimap::BiMap;
use serenity::model::prelude::UserId;
use songbird::events::context_data::{SpeakingUpdateData, VoiceData};
use songbird::model::payload::{ClientDisconnect, Speaking};
use songbird::{Call, CoreEvent, Event, EventContext, EventHandler as VoiceEventHandler};
use std::collections::{HashMap, LinkedList};
use std::sync::Arc;
use tokio::sync::*;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct ReceivingVoiceInfo {
    pub client_user_id: UserId,
}

#[derive(Clone)]
pub struct ReceivingVoiceContainer {
    client_voice: Arc<RwLock<Voice>>,
}

#[async_trait]
impl<'a> VoiceContainer<'a> for ReceivingVoiceContainer {
    type Voice = RwLockReadGuard<'a, Voice>;
    async fn voice(&'a self) -> Self::Voice {
        self.client_voice.read().await
    }
    fn blocking_voice(&'a self) -> Self::Voice {
        self.client_voice.blocking_read()
    }
}

#[derive(Clone)]
pub struct VoiceReceiverConfiguration {
    pub queue_size: usize,
    pub cut_voice_chunks_size: usize,
}

impl Default for VoiceReceiverConfiguration {
    fn default() -> Self {
        Self {
            queue_size: 25,
            cut_voice_chunks_size: 1000,
        }
    }
}

#[derive(Clone)]
pub struct VoiceReceiver {
    configuration: Arc<VoiceReceiverConfiguration>,
    ids_map: Arc<RwLock<BiMap<u32, UserId>>>,
    queue_clients_voices: Arc<Mutex<LinkedList<Arc<RwLock<Voice>>>>>,
    processing_clients_voices: Arc<Mutex<HashMap<u32, Arc<RwLock<Voice>>>>>,
}

impl VoiceReceiver {
    pub fn with_configuration(configuration: VoiceReceiverConfiguration) -> VoiceReceiver {
        Self {
            configuration: Arc::new(configuration),
            ids_map: Arc::new(Default::default()),
            queue_clients_voices: Arc::new(Default::default()),
            processing_clients_voices: Arc::new(Default::default()),
        }
    }

    pub fn subscribe(&self, handler: &mut Call) {
        handler.add_global_event(CoreEvent::SpeakingStateUpdate.into(), self.clone());
        handler.add_global_event(CoreEvent::SpeakingUpdate.into(), self.clone());
        handler.add_global_event(CoreEvent::VoicePacket.into(), self.clone());
        handler.add_global_event(CoreEvent::ClientDisconnect.into(), self.clone());
        handler.add_global_event(CoreEvent::DriverConnect.into(), self.clone());
        handler.add_global_event(CoreEvent::DriverDisconnect.into(), self.clone());
        handler.add_global_event(CoreEvent::DriverReconnect.into(), self.clone());
    }

    pub async fn next_voice(
        &self,
    ) -> Option<InfoVoiceContainer<ReceivingVoiceInfo, ReceivingVoiceContainer>> {
        let ids_map = self.ids_map.read().await;
        let mut queue_clients_voices = self.queue_clients_voices.lock().await;
        let mut voices_to_revert: Vec<Arc<RwLock<Voice>>> = vec![];
        let mut voice_container_to_return: Option<
            InfoVoiceContainer<ReceivingVoiceInfo, ReceivingVoiceContainer>,
        > = None;
        while let Some(client_voice) = queue_clients_voices.pop_front() {
            let client_voice_id = client_voice.read().await.id;
            if let Some(client_user_id) = ids_map.get_by_left(&client_voice_id) {
                let voice_container = InfoVoiceContainer {
                    info: ReceivingVoiceInfo {
                        client_user_id: *client_user_id,
                    },
                    container: ReceivingVoiceContainer { client_voice },
                };
                voice_container_to_return = Some(voice_container);
                break;
            } else {
                voices_to_revert.push(client_voice);
            }
        }
        for voice_to_revert in voices_to_revert {
            queue_clients_voices.push_back(voice_to_revert);
        }
        voice_container_to_return
    }

    async fn create_voice_in_queue(&self, ssrc: u32) -> Arc<RwLock<Voice>> {
        let mut queue_clients_voices = self.queue_clients_voices.lock().await;
        let client_voice = Voice {
            id: ssrc,
            chunks: vec![],
            is_completed: false,
        };
        let client_voice = Arc::new(RwLock::new(client_voice));
        if queue_clients_voices.len() >= self.configuration.queue_size {
            queue_clients_voices.pop_front();
        }
        queue_clients_voices.push_back(client_voice.clone());
        client_voice
    }

    async fn update_for_speaking(&self, speaking: &Speaking) {
        let mut ids_map = self.ids_map.write().await;
        if let Some(user_id) = speaking.user_id {
            ids_map.insert(speaking.ssrc, UserId(user_id.0));
        } else {
            ids_map.remove_by_left(&speaking.ssrc);
        }
    }

    async fn update_for_speaking_update_data(&self, data: &SpeakingUpdateData) {
        let mut processing_clients_voices = self.processing_clients_voices.lock().await;
        if let Some(processing_client_voice) = processing_clients_voices.remove(&data.ssrc) {
            if !data.speaking {
                processing_client_voice.write().await.is_completed = true;
            } else {
                processing_clients_voices.insert(data.ssrc, processing_client_voice);
            }
        } else if data.speaking {
            let client_voice = self.create_voice_in_queue(data.ssrc).await;
            processing_clients_voices.insert(data.ssrc, client_voice);
        }
    }

    async fn update_for_voice_data<'a>(&self, data: &VoiceData<'a>) {
        let Some(audio) = data.audio else {
            return;
        };
        let mut processing_clients_voices = self.processing_clients_voices.lock().await;
        let Some(processing_client_voice) = processing_clients_voices.get(&data.packet.ssrc) else {
            return;
        };
        let mut processing_client_voice = processing_client_voice.write().await;
        processing_client_voice.chunks.push(audio.clone());
        if processing_client_voice.chunks.len() >= self.configuration.cut_voice_chunks_size {
            processing_client_voice.is_completed = true;
            drop(processing_client_voice);
            let client_voice = self.create_voice_in_queue(data.packet.ssrc).await;
            processing_clients_voices.insert(data.packet.ssrc, client_voice);
        }
    }

    async fn update_for_disconnect(&self, disconnect: &ClientDisconnect) {
        let mut processing_clients_voices = self.processing_clients_voices.lock().await;
        let mut ids_map = self.ids_map.write().await;
        let Some((ssrc, _)) = ids_map.remove_by_right(&UserId(disconnect.user_id.0)) else {
            return;
        };
        let Some(processing_client_voice) = processing_clients_voices.remove(&ssrc) else {
            return;
        };
        processing_client_voice.write().await.is_completed = true;
    }

    async fn reset_in_processing(&self) {
        let mut processing_clients_voices = self.processing_clients_voices.lock().await;
        for (_, processing_client_voice) in processing_clients_voices.iter() {
            processing_client_voice.write().await.is_completed = true;
        }
        processing_clients_voices.clear();
    }
}

#[async_trait]
impl QueuedItemsContainer for VoiceReceiver {
    type Item = InfoVoiceContainer<ReceivingVoiceInfo, ReceivingVoiceContainer>;
    async fn next(&self) -> Option<Self::Item> {
        self.next_voice().await
    }
}

#[async_trait]
impl VoiceEventHandler for VoiceReceiver {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        use EventContext as Ctx;
        let event_uuid = Uuid::new_v4();
        match ctx {
            Ctx::SpeakingStateUpdate(speaking) => {
                debug!(
                    "[{}] VoiceEvent ENTER SpeakingStateUpdate: {:?}.",
                    event_uuid, speaking
                );
                self.update_for_speaking(speaking).await;
                debug!("[{}] VoiceEvent EXIT SpeakingStateUpdate.", event_uuid);
            }
            Ctx::SpeakingUpdate(data) => {
                debug!(
                    "[{}] VoiceEvent ENTER SpeakingUpdate: {:?}.",
                    event_uuid, data
                );
                self.update_for_speaking_update_data(data).await;
                debug!("[{}] VoiceEvent EXIT SpeakingUpdate.", event_uuid);
            }
            Ctx::VoicePacket(data) => {
                debug!("[{}] VoiceEvent ENTER VoicePacket: {:?}.", event_uuid, data);
                self.update_for_voice_data(data).await;
                debug!("[{}] VoiceEvent EXIT VoicePacket.", event_uuid);
            }
            Ctx::ClientDisconnect(disconnect) => {
                debug!(
                    "[{}] VoiceEvent ENTER ClientDisconnect: {:?}.",
                    event_uuid, disconnect
                );
                self.update_for_disconnect(disconnect).await;
                debug!("[{}] VoiceEvent EXIT ClientDisconnect.", event_uuid);
            }
            Ctx::DriverConnect(data) => {
                debug!(
                    "[{}] VoiceEvent ENTER DriverConnect: {:?}.",
                    event_uuid, data
                );
                self.reset_in_processing().await;
                debug!("[{}] VoiceEvent EXIT DriverConnect.", event_uuid);
            }
            Ctx::DriverDisconnect(data) => {
                debug!(
                    "[{}] VoiceEvent ENTER DriverDisconnect: {:?}.",
                    event_uuid, data
                );
                self.reset_in_processing().await;
                debug!("[{}] VoiceEvent EXIT DriverDisconnect.", event_uuid);
            }
            Ctx::DriverReconnect(data) => {
                debug!(
                    "[{}] VoiceEvent ENTER DriverReconnect: {:?}.",
                    event_uuid, data
                );
                self.reset_in_processing().await;
                debug!("[{}] VoiceEvent EXIT DriverReconnect.", event_uuid);
            }
            _ => unimplemented!(),
        }
        None
    }
}
