mod commands;
mod commands_handler;
mod guilds_voices_receivers;
mod info_voice_container;
mod queued_items_container;
mod recognition;
mod recognizer;
mod red_alert_handler;
mod voice;
mod voice_config;
mod voice_receiver;

use commands::*;
use commands_handler::*;
use guilds_voices_receivers::*;
use info_voice_container::*;
use queued_items_container::*;
use recognition::*;
use recognizer::*;
use red_alert_handler::*;
use voice::*;
use voice_config::*;
use voice_receiver::*;

use async_trait::async_trait;
use config::{Config as ConfigFile, File};
use serenity::model::gateway::Activity;
use serenity::model::id::GuildId;
use serenity::model::prelude::{ChannelId, OnlineStatus, Ready, UserId};
use serenity::prelude::{Context, GatewayIntents, Mentionable};
use serenity::Client;
use songbird::driver::DecodeMode;
use songbird::{Config, SerenityInit};
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::os::raw::c_int;
use std::path::Path;
use std::sync::Arc;
use voskrust::api::{set_log_level as set_vosk_log_level, Model as VoskModel};

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() {
    let _ = log4rs::init_file("log_config.yaml", Default::default());

    let settings = ConfigFile::builder()
        .add_source(File::from(Path::new("config.yaml")))
        .build()
        .expect("You should setup file \"config.yaml\"!");

    let token = settings
        .get_string("discord_token")
        .expect("Expected a token in the config!");

    let listening_text = settings.get_string("listening_text").ok();

    let vosk_model_path = settings
        .get_string("vosk_model_path")
        .expect("Expected a VOSK model path in the config!");
    let vosk_log_level = settings.get_int("vosk_log_level");

    let voice_settings = settings
        .get_table("voice")
        .expect("Expected a voice configuration in the config!");

    let target_words = voice_settings
        .get("target_words")
        .expect("Expected a target words in the config!")
        .clone();
    let target_words: Vec<String> = target_words
        .try_deserialize()
        .expect("Incorrect format of target words in the config!");

    let self_words = voice_settings
        .get("self_words")
        .expect("Expected a self words in the config!")
        .clone();
    let self_words: Vec<String> = self_words
        .try_deserialize()
        .expect("Incorrect format of self words in the config!");

    let aliases = voice_settings
        .get("aliases")
        .expect("Expected a aliases in the config!")
        .clone();
    let aliases: HashMap<String, u64> = aliases
        .try_deserialize()
        .expect("Incorrect format of aliases in the config!");

    let similarity_threshold = voice_settings
        .get("similarity_threshold")
        .expect("Expected a similarity threshold in the config!")
        .clone();
    let similarity_threshold: f32 = similarity_threshold
        .try_deserialize()
        .expect("Incorrect format of similarity threshold in the config!");

    if let Ok(vosk_log_level) = vosk_log_level {
        set_vosk_log_level(vosk_log_level as c_int);
    }

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Into::<Handler>::into(CommandsHandlerConstructor {
            recognition_model: VoskModel::new(vosk_model_path.as_str())
                .expect("Incorrect recognition model!"),
            config: RedAlertCommandsConfig {
                listening_text,
                voice: VoiceConfig {
                    target_words,
                    self_words,
                    aliases,
                    similarity_threshold,
                },
            },
            red_alert_handler: Arc::new(RedAlertHandler),
        }))
        .register_songbird_from_config(Config::default().decode_mode(DecodeMode::Decode))
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
