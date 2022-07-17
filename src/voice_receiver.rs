use bimap::BiMap;
use serenity::async_trait;
use serenity::model::prelude::UserId;
use songbird::events::context_data::{SpeakingUpdateData, VoiceData};
use songbird::model::payload::{ClientDisconnect, Speaking};
use songbird::{Call, CoreEvent, Event, EventContext, EventHandler as VoiceEventHandler};
use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard};

pub struct ReadVoiceContainer {
    client_user_id: UserId,
    client_voice: Arc<RwLock<ClientVoice>>,
}

impl ReadVoiceContainer {
    pub fn user_id(&self) -> UserId {
        self.client_user_id
    }
    pub fn read_lock(&self) -> RwLockReadGuard<ClientVoice> {
        self.client_voice.read().unwrap()
    }
}

pub struct ClientVoice {
    pub id: u32,
    pub chunks: Vec<Vec<i16>>,
    pub is_completed: bool,
}

impl ClientVoice {
    fn empty_for_id(id: u32) -> Self {
        ClientVoice {
            id,
            chunks: vec![],
            is_completed: false,
        }
    }
}

#[derive(Clone)]
pub struct VoiceReceiver {
    ids_map: Arc<RwLock<BiMap<u32, UserId>>>,
    for_taking_clients_voices: Arc<RwLock<Vec<Arc<RwLock<ClientVoice>>>>>,
    in_processing_clients_voices: Arc<RwLock<HashMap<u32, Arc<RwLock<ClientVoice>>>>>,
}

impl VoiceReceiver {
    pub fn start_on(handler: &mut Call) -> VoiceReceiver {
        let voice_receiver = VoiceReceiver {
            ids_map: Arc::new(Default::default()),
            for_taking_clients_voices: Arc::new(Default::default()),
            in_processing_clients_voices: Arc::new(Default::default()),
        };

        handler.add_global_event(
            CoreEvent::SpeakingStateUpdate.into(),
            voice_receiver.clone(),
        );
        handler.add_global_event(CoreEvent::SpeakingUpdate.into(), voice_receiver.clone());
        handler.add_global_event(CoreEvent::VoicePacket.into(), voice_receiver.clone());
        handler.add_global_event(CoreEvent::ClientDisconnect.into(), voice_receiver.clone());
        handler.add_global_event(CoreEvent::DriverConnect.into(), voice_receiver.clone());
        handler.add_global_event(CoreEvent::DriverDisconnect.into(), voice_receiver.clone());
        handler.add_global_event(CoreEvent::DriverReconnect.into(), voice_receiver.clone());

        voice_receiver
    }

    pub fn next_voice(&self) -> Option<ReadVoiceContainer> {
        let ids_map = self.ids_map.read().unwrap();
        let mut clients_voices = self.for_taking_clients_voices.write().unwrap();
        loop {
            if clients_voices.len() > 0 {
                let client_voice = clients_voices.remove(0);
                let client_voice_id = client_voice.read().unwrap().id;
                if let Some(client_user_id) = ids_map.get_by_left(&client_voice_id) {
                    return Some(ReadVoiceContainer {
                        client_user_id: client_user_id.clone(),
                        client_voice,
                    });
                } else {
                    continue;
                }
            } else {
                return None;
            }
        }
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
        let mut in_processing_clients_voices = self.in_processing_clients_voices.write().unwrap();
        if let Some(in_processing_client_voice) = in_processing_clients_voices.remove(&data.ssrc) {
            if !data.speaking {
                let mut in_processing_client_voice = in_processing_client_voice.write().unwrap();
                in_processing_client_voice.is_completed = true
            } else {
                in_processing_clients_voices.insert(data.ssrc, in_processing_client_voice);
            }
        } else {
            if data.speaking {
                let mut for_taking_clients_voices = self.for_taking_clients_voices.write().unwrap();
                let client_voice = ClientVoice::empty_for_id(data.ssrc);
                let client_voice = Arc::new(RwLock::new(client_voice));
                in_processing_clients_voices.insert(data.ssrc, client_voice.clone());
                for_taking_clients_voices.push(client_voice);
            }
        }
    }

    fn update_for_voice_data(&self, data: &VoiceData) {
        if let Some(audio) = data.audio {
            let in_processing_clients_voices = self.in_processing_clients_voices.read().unwrap();
            if let Some(in_processing_client_voice) =
                in_processing_clients_voices.get(&data.packet.ssrc)
            {
                let mut in_processing_client_voice = in_processing_client_voice.write().unwrap();
                in_processing_client_voice.chunks.push(audio.clone());
            }
        }
    }

    fn update_for_disconnect(&self, disconnect: &ClientDisconnect) {
        let mut in_processing_clients_voices = self.in_processing_clients_voices.write().unwrap();
        let mut ids_map = self.ids_map.write().unwrap();
        if let Some((ssrc, _)) = ids_map.remove_by_right(&UserId(disconnect.user_id.0)) {
            if let Some(in_processing_client_voice) = in_processing_clients_voices.remove(&ssrc) {
                let mut in_processing_client_voice = in_processing_client_voice.write().unwrap();
                in_processing_client_voice.is_completed = true
            }
        }
    }

    fn reset_in_processing(&self) {
        let mut in_processing_clients_voices = self.in_processing_clients_voices.write().unwrap();
        for (_, in_processing_client_voice) in in_processing_clients_voices.iter() {
            let mut in_processing_client_voice = in_processing_client_voice.write().unwrap();
            in_processing_client_voice.is_completed = true
        }
        in_processing_clients_voices.clear();
    }
}

#[async_trait]
impl VoiceEventHandler for VoiceReceiver {
    #[allow(unused_variables)]
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        use EventContext as Ctx;
        match ctx {
            Ctx::SpeakingStateUpdate(speaking) => self.update_for_speaking(speaking),
            Ctx::SpeakingUpdate(data) => self.update_for_speaking_update_data(data),
            Ctx::VoicePacket(data) => self.update_for_voice_data(data),
            Ctx::ClientDisconnect(disconnect) => self.update_for_disconnect(disconnect),
            Ctx::DriverConnect(data) => self.reset_in_processing(),
            Ctx::DriverDisconnect(data) => self.reset_in_processing(),
            Ctx::DriverReconnect(data) => self.reset_in_processing(),
            _ => unimplemented!(),
        }
        None
    }
}
