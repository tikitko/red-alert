use crate::voices_storage::{StorageVoice, VoicesStorage};
use guard::guard;
use serenity::model::id::GuildId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct GuildsVoicesStorage<S: VoicesStorage<Information = ()>>(
    pub Arc<RwLock<HashMap<GuildId, S>>>,
);

#[derive(Clone, Copy)]
pub struct GuildsVoicesStorageInformation {
    pub guild_id: GuildId,
}

impl<S: VoicesStorage<Information = ()>> VoicesStorage for GuildsVoicesStorage<S> {
    type Information = GuildsVoicesStorageInformation;
    fn next_voice(&self) -> Option<StorageVoice<Self::Information>> {
        let guilds_voices_storages = self.0.read().unwrap();
        for (guild_id, voice_storage) in guilds_voices_storages.iter() {
            guard!(let Some(voice) = voice_storage.next_voice()
                else { continue });
            return Some(StorageVoice {
                information: GuildsVoicesStorageInformation {
                    guild_id: guild_id.clone(),
                },
                container: voice.container,
            });
        }
        None
    }
}
