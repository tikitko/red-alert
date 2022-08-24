use serenity::model::prelude::UserId;
use std::sync::{Arc, RwLock, RwLockReadGuard};

pub struct StorageVoice<I: Copy> {
    pub information: I,
    pub container: ReadVoiceContainer,
}

pub struct ReadVoiceContainer {
    client_user_id: UserId,
    client_voice: Arc<RwLock<ClientVoice>>,
}

impl ReadVoiceContainer {
    pub fn new(client_user_id: UserId, client_voice: Arc<RwLock<ClientVoice>>) -> Self {
        Self {
            client_user_id,
            client_voice,
        }
    }
    pub fn user_id(&self) -> UserId {
        self.client_user_id
    }
    pub fn voice(&self) -> RwLockReadGuard<ClientVoice> {
        self.client_voice.read().unwrap()
    }
}

pub struct ClientVoice {
    pub id: u32,
    pub chunks: Vec<Vec<i16>>,
    pub is_completed: bool,
}

pub trait VoicesStorage: Clone + Send + Sync {
    type Information: Clone + Copy + Send + Sync;
    fn next_voice(&self) -> Option<StorageVoice<Self::Information>>;
}
