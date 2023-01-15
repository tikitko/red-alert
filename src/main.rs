#![allow(dead_code)]

mod components;
mod red_alert;

#[macro_use]
extern crate log;

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate fluent;

#[tokio::main]
async fn main() {
    use config::{Config, File};
    use serenity::prelude::GatewayIntents;
    use serenity::Client;
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

    let lang_id_string = settings
        .get_string("lang_id")
        .expect("Expected a lang id in the config!");

    let l10n = components::L10n::load(&lang_id_string);

    let vosk_model_path = settings
        .get_string("vosk_model_path")
        .expect("Expected a VOSK model path in the config!");
    let vosk_log_level = settings.get_int("vosk_log_level");

    if let Ok(vosk_log_level) = vosk_log_level {
        set_vosk_log_level(vosk_log_level as c_int);
    }

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(
            red_alert::RedAlertCommandsHandlerConstructor {
                recognition_model: VoskModel::new(vosk_model_path.as_str())
                    .expect("Incorrect recognition model!"),
                red_alert_handler: Arc::new(red_alert::RedAlertHandler),
                l10n,
            }
            .build(),
        )
        .register_songbird_from_config(SongbirdConfig::default().decode_mode(DecodeMode::Decode))
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
