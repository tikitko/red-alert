use fon::chan::Ch16;
use fon::Audio;
use serenity::async_trait;
use serenity::model::prelude::UserId;
use songbird::events::context_data::{SpeakingUpdateData, VoiceData};
use songbird::model::payload::Speaking;
use songbird::{Call, CoreEvent, Event, EventContext, EventHandler as VoiceEventHandler};
use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard};

pub struct ReadVoiceContainer {
    client_user_id: Option<UserId>,
    client_voice: Arc<RwLock<ClientVoice>>,
}

impl ReadVoiceContainer {
    pub fn user_id(&self) -> Option<UserId> {
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
    output_hz: u32,
    ids_map: Arc<RwLock<HashMap<u32, UserId>>>,
    for_taking_clients_voices: Arc<RwLock<Vec<Arc<RwLock<ClientVoice>>>>>,
    in_processing_clients_voices: Arc<RwLock<HashMap<u32, Arc<RwLock<ClientVoice>>>>>,
}

impl VoiceReceiver {
    pub fn start_on(handler: &mut Call, output_hz: u32) -> VoiceReceiver {
        let voice_receiver = VoiceReceiver {
            output_hz,
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

        voice_receiver
    }

    pub fn default_start_on(handler: &mut Call) -> VoiceReceiver {
        Self::start_on(handler, 16_000)
    }

    pub fn output_hz(&self) -> u32 {
        self.output_hz
    }

    pub fn extract_voice(&self) -> Option<ReadVoiceContainer> {
        let ids_map = self.ids_map.read().unwrap();
        let mut clients_voices = self.for_taking_clients_voices.write().unwrap();
        if clients_voices.len() > 0 {
            let client_voice = clients_voices.remove(0).clone();
            let client_voice_id = client_voice.read().unwrap().id;
            let client_user_id = ids_map.get(&client_voice_id).cloned();
            Some(ReadVoiceContainer {
                client_user_id,
                client_voice,
            })
        } else {
            None
        }
    }

    fn update_for_speaking(&self, speaking: &Speaking) {
        // println!("update_for_speaking: {}", speaking.ssrc);
        let mut ids_map = self.ids_map.write().unwrap();
        if let Some(user_id) = speaking.user_id {
            ids_map.insert(speaking.ssrc, UserId(user_id.0));
        } else {
            ids_map.remove(&speaking.ssrc);
        }
    }

    fn update_for_speaking_update_data(&self, data: &SpeakingUpdateData) {
        // println!("update_for_speaking_update_data: {}", data.ssrc);
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
                for_taking_clients_voices.push(client_voice.clone());
            }
        }
    }

    fn update_for_voice_data(&self, data: &VoiceData) {
        // println!("update_for_voice_data: {}", data.packet.ssrc);
        if let Some(audio) = data.audio {
            let in_processing_clients_voices = self.in_processing_clients_voices.read().unwrap();
            if let Some(in_processing_client_voice) =
                in_processing_clients_voices.get(&data.packet.ssrc)
            {
                let mut in_processing_client_voice = in_processing_client_voice.write().unwrap();
                let audio = Audio::<Ch16, 2>::with_i16_buffer(48_000, audio.as_slice());
                let mut audio = Audio::<Ch16, 1>::with_audio(self.output_hz, &audio);
                let audio = audio.as_i16_slice();
                in_processing_client_voice.chunks.push(Vec::from(audio));
            }
        }
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
            _ => unimplemented!(),
        }
        None
    }
}
