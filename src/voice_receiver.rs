use crate::voices_storage::{ClientVoice, ReadVoiceContainer, StorageVoice, VoicesStorage};
use bimap::BiMap;
use guard::guard;
use serenity::async_trait;
use serenity::model::prelude::UserId;
use songbird::events::context_data::{SpeakingUpdateData, VoiceData};
use songbird::model::payload::{ClientDisconnect, Speaking};
use songbird::{Call, CoreEvent, Event, EventContext, EventHandler as VoiceEventHandler};
use std::collections::{HashMap, LinkedList};
use std::sync::{Arc, Mutex, RwLock};

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
    queue_clients_voices: Arc<Mutex<LinkedList<Arc<RwLock<ClientVoice>>>>>,
    processing_clients_voices: Arc<Mutex<HashMap<u32, Arc<RwLock<ClientVoice>>>>>,
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

    fn create_voice_in_queue(&self, ssrc: u32) -> Arc<RwLock<ClientVoice>> {
        let mut queue_clients_voices = self.queue_clients_voices.lock().unwrap();
        let client_voice = ClientVoice {
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

    fn update_for_speaking(&self, speaking: &Speaking) {
        let mut ids_map = self.ids_map.write().unwrap();
        if let Some(user_id) = speaking.user_id {
            ids_map.insert(speaking.ssrc, UserId(user_id.0));
        } else {
            ids_map.remove_by_left(&speaking.ssrc);
        }
    }

    fn update_for_speaking_update_data(&self, data: &SpeakingUpdateData) {
        let mut processing_clients_voices = self.processing_clients_voices.lock().unwrap();
        if let Some(processing_client_voice) = processing_clients_voices.remove(&data.ssrc) {
            if !data.speaking {
                processing_client_voice.write().unwrap().is_completed = true;
            } else {
                processing_clients_voices.insert(data.ssrc, processing_client_voice);
            }
        } else if data.speaking {
            let client_voice = self.create_voice_in_queue(data.ssrc);
            processing_clients_voices.insert(data.ssrc, client_voice);
        }
    }

    fn update_for_voice_data(&self, data: &VoiceData) {
        guard!(let Some(audio) = data.audio
            else { return });
        let mut processing_clients_voices = self.processing_clients_voices.lock().unwrap();
        guard!(let Some(processing_client_voice) = processing_clients_voices.get(&data.packet.ssrc)
            else { return });
        let mut processing_client_voice = processing_client_voice.write().unwrap();
        processing_client_voice.chunks.push(audio.clone());
        if processing_client_voice.chunks.len() >= self.configuration.cut_voice_chunks_size {
            processing_client_voice.is_completed = true;
            drop(processing_client_voice);
            let client_voice = self.create_voice_in_queue(data.packet.ssrc);
            processing_clients_voices.insert(data.packet.ssrc, client_voice);
        }
    }

    fn update_for_disconnect(&self, disconnect: &ClientDisconnect) {
        let mut processing_clients_voices = self.processing_clients_voices.lock().unwrap();
        let mut ids_map = self.ids_map.write().unwrap();
        guard!(let Some((ssrc, _)) = ids_map.remove_by_right(&UserId(disconnect.user_id.0))
            else { return });
        guard!(let Some(processing_client_voice) = processing_clients_voices.remove(&ssrc)
            else { return });
        processing_client_voice.write().unwrap().is_completed = true;
    }

    fn reset_in_processing(&self) {
        let mut processing_clients_voices = self.processing_clients_voices.lock().unwrap();
        for (_, processing_client_voice) in processing_clients_voices.iter() {
            processing_client_voice.write().unwrap().is_completed = true;
        }
        processing_clients_voices.clear();
    }
}

impl VoicesStorage for VoiceReceiver {
    type Information = ();
    fn next_voice(&self) -> Option<StorageVoice<Self::Information>> {
        let ids_map = self.ids_map.read().unwrap();
        let mut queue_clients_voices = self.queue_clients_voices.lock().unwrap();
        let mut voices_to_revert: Vec<Arc<RwLock<ClientVoice>>> = vec![];
        let mut read_voice_container_to_return: Option<StorageVoice<Self::Information>> = None;
        while let Some(client_voice) = queue_clients_voices.pop_front() {
            let client_voice_id = client_voice.read().unwrap().id;
            if let Some(client_user_id) = ids_map.get_by_left(&client_voice_id) {
                let read_voice_container =
                    ReadVoiceContainer::new(client_user_id.clone(), client_voice);
                read_voice_container_to_return = Some(StorageVoice {
                    information: (),
                    container: read_voice_container,
                });
                break;
            } else {
                voices_to_revert.push(client_voice);
            }
        }
        for voice_to_revert in voices_to_revert {
            queue_clients_voices.push_back(voice_to_revert);
        }
        read_voice_container_to_return
    }
}

#[async_trait]
impl VoiceEventHandler for VoiceReceiver {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        use EventContext as Ctx;
        match ctx {
            Ctx::SpeakingStateUpdate(speaking) => {
                debug!("VoiceEvent SpeakingStateUpdate: {:?}.", speaking);
                self.update_for_speaking(speaking);
            }
            Ctx::SpeakingUpdate(data) => {
                debug!("VoiceEvent SpeakingUpdate: {:?}.", data);
                self.update_for_speaking_update_data(data);
            }
            Ctx::VoicePacket(data) => {
                debug!("VoiceEvent VoicePacket: {:?}.", data);
                self.update_for_voice_data(data);
            }
            Ctx::ClientDisconnect(disconnect) => {
                debug!("VoiceEvent ClientDisconnect: {:?}.", disconnect);
                self.update_for_disconnect(disconnect);
            }
            Ctx::DriverConnect(data) => {
                debug!("VoiceEvent DriverConnect: {:?}.", data);
                self.reset_in_processing();
            }
            Ctx::DriverDisconnect(data) => {
                debug!("VoiceEvent DriverDisconnect: {:?}.", data);
                self.reset_in_processing();
            }
            Ctx::DriverReconnect(data) => {
                debug!("VoiceEvent DriverReconnect: {:?}.", data);
                self.reset_in_processing();
            }
            _ => unimplemented!(),
        }
        None
    }
}
