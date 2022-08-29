mod guilds_voices_receivers;
mod info_voice_container;
mod is_sub;
mod queued_items_container;
mod recognition;
mod recognizer;
mod red_alert_handler;
mod voice;
mod voice_config;
mod voice_receiver;

use guilds_voices_receivers::*;
use info_voice_container::*;
use is_sub::*;
use queued_items_container::*;
use recognition::*;
use recognizer::*;
use red_alert_handler::*;
use voice::*;
use voice_config::*;
use voice_receiver::*;

use async_trait::async_trait;
use config::{Config as ConfigFile, File};
use guard::guard;
use serenity::model::gateway::Activity;
use serenity::model::id::GuildId;
use serenity::model::prelude::{ChannelId, Message, OnlineStatus, Ready, UserId};
use serenity::prelude::{Context, EventHandler, GatewayIntents, Mentionable, TypeMapKey};
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

struct RecognizerData {
    #[allow(dead_code)]
    cancel_sender: tokio::sync::oneshot::Sender<()>,
    guilds_voices_receivers: Arc<tokio::sync::RwLock<HashMap<GuildId, VoiceReceiver>>>,
}

impl TypeMapKey for RecognizerData {
    type Value = Self;
}

#[derive(Clone)]
struct HandlerConfig {
    listening_text: Option<String>,
    voice: VoiceConfig,
}

struct Handler {
    recognition_model: VoskModel,
    config: HandlerConfig,
    red_alert_handler: Arc<RedAlertHandler>,
}

impl Handler {
    async fn start_recognizer(&self, ctx: &Context) {
        let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
        let guilds_voices_receivers: Arc<tokio::sync::RwLock<HashMap<GuildId, VoiceReceiver>>> =
            Arc::new(Default::default());
        let mut data = ctx.data.write().await;
        data.insert::<RecognizerData>(RecognizerData {
            cancel_sender: tx,
            guilds_voices_receivers: guilds_voices_receivers.clone(),
        });

        let red_alert_handler = self.red_alert_handler.clone();
        let recognition_model = self.recognition_model.clone();
        let voice_config = self.config.voice.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let mut recognizer_signal = Recognizer {
                model: recognition_model,
                voices_queue: GuildsVoicesReceivers(guilds_voices_receivers),
            }
            .start();
            let mut session_kicked: HashSet<UserId> = HashSet::new();
            loop {
                guard!(let Some(recognizer_state) = tokio::select! {
                    recognizer_state = recognizer_signal.recv() => recognizer_state,
                    _ = &mut rx => None,
                }
                    else { break });

                let log_prefix = match recognizer_state {
                    RecognizerState::RecognitionStart(info)
                    | RecognizerState::RecognitionResult(info, _)
                    | RecognizerState::RecognitionEnd(info) => {
                        let mut prefix_parts: Vec<String> = vec![];
                        let guild_id = info.inner.guild_id;
                        if let Some(guild) = ctx.cache.guild(guild_id) {
                            prefix_parts.push(format!("[G:{}]", guild.name));
                        } else {
                            prefix_parts.push(format!("[GID:{}]", guild_id));
                        }
                        let user_id = info.user_id;
                        if let Some(user) = ctx.cache.user(user_id) {
                            prefix_parts.push(format!(
                                "[U:{}#{:04}]",
                                user.name,
                                user.discriminator.min(9999).max(1)
                            ));
                        } else {
                            prefix_parts.push(format!("[UID:{}]", user_id));
                        }
                        prefix_parts.join("")
                    }
                };
                match recognizer_state {
                    RecognizerState::RecognitionResult(info, result) => {
                        info!(
                            "{} Recognition RESULT: type: {:?}, text: \"{}\".",
                            log_prefix, result.result_type, result.text
                        );
                        guard!(let Some(kick_user_id) = voice_config.should_kick(
                            info.user_id,
                            &result.text
                        )
                            else { continue });
                        if session_kicked.contains(&kick_user_id) {
                            info!(
                                "{} Recognition RESULT skipped. User already kicked.",
                                log_prefix
                            );
                            continue;
                        }
                        info!(
                            "{} Recognition RESULT will be used for kick. Have restrictions.",
                            log_prefix
                        );
                        session_kicked.insert(kick_user_id);
                        let red_alert_handler = red_alert_handler.clone();
                        let ctx = ctx.clone();
                        tokio::spawn(async move {
                            let red_alert_deportations_results = red_alert_handler
                                .handle(&ctx, info.inner.guild_id, vec![kick_user_id])
                                .await;
                            let red_alert_deportation_result =
                                red_alert_deportations_results.get(&kick_user_id).unwrap();
                            info!(
                                "{} Recognition RESULT used for kick, status is {:?}.",
                                log_prefix, red_alert_deportation_result
                            );
                        });
                    }
                    RecognizerState::RecognitionStart(info) => {
                        info!("{} Recognition STARTED.", log_prefix);
                        session_kicked.remove(&info.user_id);
                    }
                    RecognizerState::RecognitionEnd(info) => {
                        info!("{} Recognition ENDED.", log_prefix);
                        session_kicked.remove(&info.user_id);
                    }
                }
            }
        });
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        };
        guard!(let Some(guild_id) = msg.guild_id else {
            return
        });
        let message = msg.content.to_lowercase();
        let message_words: Vec<&str> = message.split(char::is_whitespace).collect();

        let answer_msg = if message_words.is_sub(&vec!["отслеживать", "код", "красный"])
        {
            let possible_channel_id: Option<ChannelId> = {
                let mut possible_channel_id: Option<u64> = None;
                for message_word in message_words {
                    if let Ok(value) = message_word.parse::<u64>() {
                        possible_channel_id = Some(value);
                        break;
                    }
                }
                possible_channel_id.map(ChannelId)
            };
            if let Some(possible_channel_id) = possible_channel_id {
                listen_for_red_alert(&ctx, guild_id, possible_channel_id).await
            } else {
                format!("ЧТО ОТСЛЕЖИВАТЬ НАРКОМАН?")
            }
        } else if message_words.is_sub(&vec!["прекратить", "код", "красный"]) {
            exit_for_red_alert(&ctx, guild_id).await
        } else if message_words.is_sub(&vec!["код", "красный"]) {
            let author_id = msg.author.id;
            let target_users_ids: Vec<UserId> = msg.mentions.iter().map(|u| u.id).collect();
            process_red_alert(
                self.red_alert_handler.clone(),
                &ctx,
                guild_id,
                author_id,
                target_users_ids,
            )
            .await
        } else {
            format!("")
        };
        if !answer_msg.is_empty() {
            let _ = msg.channel_id.say(&ctx, answer_msg).await;
        }
    }
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
        let activity = self
            .config
            .listening_text
            .as_ref()
            .map(|t| Activity::listening(t));
        ctx.set_presence(activity, OnlineStatus::Online).await;
        self.start_recognizer(&ctx).await;
    }
}

async fn listen_for_red_alert(ctx: &Context, guild_id: GuildId, channel_id: ChannelId) -> String {
    let channel_name = channel_id.mention();
    let data = ctx.data.read().await;
    if let Some(recognizer_data) = data.get::<RecognizerData>() {
        let manager = songbird::get(ctx)
            .await
            .expect("Songbird Voice client placed in at initialisation.")
            .clone();

        let (handler_lock, conn_result) = manager.join(guild_id, channel_id).await;

        if conn_result.is_ok() {
            let mut handler = handler_lock.lock().await;

            let voice_receiver = VoiceReceiver::with_configuration(Default::default());
            voice_receiver.subscribe(handler.deref_mut());

            let mut guilds_voices_receivers = recognizer_data.guilds_voices_receivers.write().await;
            guilds_voices_receivers.insert(guild_id, voice_receiver);

            format!("ОТСЛЕЖИВАЮ КОД КРАСНЫЙ В КАНАЛЕ {channel_name}...")
        } else {
            format!("ОШИБКА СЛЕЖКИ ЗА КАНАЛОМ {channel_name}. НЕ ПОЛУЧАЕТСЯ ВОЙТИ В КАНАЛ...")
        }
    } else {
        format!("ОШИБКА СЛЕЖКИ ЗА КАНАЛОМ {channel_name}. РАСПОЗНАВАТЕЛИ РЕЧИ МЕРТВЫ...")
    }
}

async fn exit_for_red_alert(ctx: &Context, guild_id: GuildId) -> String {
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        let data = ctx.data.read().await;
        if manager.remove(guild_id).await.is_err() {
            format!("ПРОИЗОШЛА ОШИБКА! НЕ ПОЛУЧАЕТСЯ ОТКЛЮЧИТЬСЯ...")
        } else if let Some(recognizer_data) = data.get::<RecognizerData>() {
            let mut guilds_voices_receivers = recognizer_data.guilds_voices_receivers.write().await;
            guilds_voices_receivers.remove(&guild_id);

            format!("ПРЕКРАЩАЮ ОТСЛЕЖИВАНИЕ КАНАЛА!")
        } else {
            format!("ПРОИЗОШЛА ОШИБКА! РАСПОЗНАВАТЕЛИ РЕЧИ МЕРТВЫ...")
        }
    } else {
        format!("НЕ ОТСЛЕЖИВАЮ КАНАЛЫ!")
    }
}

async fn process_red_alert(
    red_alert_handler: Arc<RedAlertHandler>,
    ctx: &Context,
    guild_id: GuildId,
    author_user_id: UserId,
    target_users_ids: Vec<UserId>,
) -> String {
    let red_alert_result = red_alert_handler
        .handle(ctx, guild_id, target_users_ids)
        .await;

    match red_alert_result.len() {
        0 => {
            if let Some(result) = red_alert_handler
                .handle(ctx, guild_id, vec![author_user_id])
                .await
                .get(&author_user_id)
            {
                match result {
                    RedAlertDeportationResult::Deported => format!("ВИЖУ ТЫ ЗАБЫЛ УКАЗАТЬ ЦЕЛЬ ДЛЯ КРАСНОГО КОДА, НИЧЕГО... ШМАЛЬНЕМ В ТЕБЯ! (ИСПОЛЬЗУЙ ТЕГИ) ПРИНЯТО К ИСПОЛНЕНИЮ!"),
                    RedAlertDeportationResult::NotFound => format!(":face_with_monocle: ПОЛЬЗУЙСЯ ТЕГАМИ, И ЛУЧШЕ НЕ ЗАХОДИ В КАНАЛ, А ТО КИКНУ С ТАКИМИ ПРИКОЛАМИ! Пшшшш..."),
                    RedAlertDeportationResult::Error(_) => format!("СЛОМАЛСЯ ПОКА ПЫТАЛСЯ ТЕБЯ КИКНУТЬ ЧТО НЕПРАВИЛЬНОЕ ИСПОЛЬЗОВАНИЕ, КАК ВСЕГДА КОД ГОВНА! ОТМЕНА! Пшшшш...")
                }
            } else {
                format!("КАНДИДАТ НА КОД КРАСНЫЙ НЕ УКАЗАН! ОТМЕНА ОПЕРАЦИИ! Пшшшш...")
            }
        }
        1 => {
            if let Some((user_id, deport_status)) = red_alert_result.iter().next() {
                let user_name = user_id.mention();
                match deport_status {
                    RedAlertDeportationResult::Deported => format!("КОД КРАСНЫЙ ПОДТВЕРЖДЕН! АНТИКРИНЖ ОРУЖИЕ ИСПОЛЬЗОВАНО ПРОТИВ {user_name}!!! 0)00))00"),
                    RedAlertDeportationResult::NotFound => {
                        if let Some(result) = red_alert_handler.handle(
                            ctx,
                            guild_id,
                            vec![author_user_id]
                        )
                            .await
                            .get(&author_user_id)
                        {
                            match result {
                                RedAlertDeportationResult::Deported => format!("В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЗНАЧИТ У ТЕБЯ БЕДЫ С БОШКОЙ, КОД КРАСНЫЙ НА ТЕБЯ!"),
                                RedAlertDeportationResult::NotFound => format!("ДОФИГА УМНЫЙ ВИЖУ? В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЖАЛЬ ТЕБЯ В КАНАЛЕ НЕТУ, ТАК БЫ ТЕБЯ ШМАЛЬНУЛ КОДОМ КРАСНЫМ! ОТМЕНА! Пшшшш..."),
                                RedAlertDeportationResult::Error(_) => format!("ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ТО ЧТО ТЫ ПЫТАЛСЯ КИКНУТЬ ТОГО КОГО НЕТ, НО Я СЛОМАЛСЯ! Пшшшш...")
                            }
                        } else {
                            format!("КАНДИДАТ НА КОД КРАСНЫЙ НЕ НАЙДЕН! ОТМЕНА ОПЕРАЦИИ! Пшшшш...")
                        }
                    }
                    RedAlertDeportationResult::Error(_) => {
                        format!("АУЧ, МАСЛИНУ ПОЙМАЛ, ОШИБКА В СИСТЕМЕё0))")
                    }
                }
            } else {
                format!("ШТО ПРОИСХОДИТ))0)0 ТЫ МЕНЯ ПОЧТИ СЛОМАЛ))) НО САСАЙ)")
            }
        }
        _ => {
            let mut some_kicked = false;
            let mut result_strings = Vec::new();
            for (user_id, deport_status) in red_alert_result {
                let user_name = user_id.mention();
                let deport_status = match deport_status {
                    RedAlertDeportationResult::Deported => {
                        some_kicked = true;
                        "ИСПОЛНЕНО"
                    }
                    RedAlertDeportationResult::NotFound => "НЕ В КАНАЛЕ",
                    RedAlertDeportationResult::Error(_) => "ОШИБКА (ПРОЧНЫЙ СУ*А)",
                };
                result_strings.push(format!("{user_name} СТАТУС: {deport_status}"))
            }
            let result_string = result_strings.join("\n");
            if some_kicked {
                format!("ОУ, МАССОВЫЙ КОД КРАСНЫЙ? СТАТУС ВЫКОСА КРИНЖОВИКОВ:\n{result_string}")
            } else {
                if let Some(result) = red_alert_handler
                    .handle(ctx, guild_id, vec![author_user_id])
                    .await
                    .get(&author_user_id)
                {
                    match result {
                        RedAlertDeportationResult::Deported => format!("МАССОВЫЙ КОД КРАСНЫЙ ШТУКА ОПАСНАЯ, ТАК КАК ПО РАЗНЫМ ПРИЧИНАМ Я НИКОГО НЕ КИКНУЛ, КИКНУ ТЕБЯ )В)В)))0"),
                        RedAlertDeportationResult::NotFound => format!("ЖАЛЬ ТЕБЯ НЕ МОГУ ПРШИТЬ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ! ОТМЕНА Пшшшш..."),
                        RedAlertDeportationResult::Error(_) => format!("ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ, НО САМ ОБО****СЯ! Пшшшш...")
                    }
                } else {
                    format!("ПОЛНЫЙ ПРОВАЛ КОДА КРАСНОГО! ОТМЕНА ОПЕРАЦИИ Пшшшш...")
                }
            }
        }
    }
}

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
        .event_handler(Handler {
            recognition_model: VoskModel::new(vosk_model_path.as_str())
                .expect("Incorrect recognition model!"),
            config: HandlerConfig {
                listening_text,
                voice: VoiceConfig {
                    target_words,
                    self_words,
                    aliases,
                    similarity_threshold,
                },
            },
            red_alert_handler: Arc::new(RedAlertHandler),
        })
        .register_songbird_from_config(Config::default().decode_mode(DecodeMode::Decode))
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
