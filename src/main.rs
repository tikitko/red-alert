#![allow(dead_code)]

mod commands_handler;
mod guilds_voices_receivers;
mod info_voice_container;
mod queued_items_container;
mod recognition;
mod recognizer;
mod red_alert_commands_handler;
mod red_alert_handler;
mod voice;
mod voice_config;
mod voice_receiver;

use commands_handler::*;
use guilds_voices_receivers::*;
use info_voice_container::*;
use queued_items_container::*;
use recognition::*;
use recognizer::*;
use red_alert_commands_handler::*;
use red_alert_handler::*;
use serenity::Client;
use voice::*;
use voice_config::*;
use voice_receiver::*;

#[macro_use]
extern crate log;

#[macro_use]
extern crate async_trait;

#[tokio::main]
async fn main() {
    use config::{Config, File};
    use serenity::prelude::GatewayIntents;
    use songbird::driver::DecodeMode;
    use songbird::Config as SongbirdConfig;
    use songbird::SerenityInit;
    use std::os::raw::c_int;
    use std::path::Path;
    use std::sync::Arc;
    use voskrust::api::{set_log_level as set_vosk_log_level, Model as VoskModel};

    let _ = log4rs::init_file("log_config.yaml", Default::default());

    let settings = Config::builder()
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

    if let Ok(vosk_log_level) = vosk_log_level {
        set_vosk_log_level(vosk_log_level as c_int);
    }

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Into::<Handler>::into(CommandsHandlerConstructor {
            recognition_model: VoskModel::new(vosk_model_path.as_str())
                .expect("Incorrect recognition model!"),
            listening_text,
            red_alert_handler: Arc::new(RedAlertHandler),
        }))
        .register_songbird_from_config(SongbirdConfig::default().decode_mode(DecodeMode::Decode))
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
