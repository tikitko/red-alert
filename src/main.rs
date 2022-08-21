mod recognition;
mod recognizer;
mod red_alert_handler;
mod voice_receiver;

use recognizer::*;
use red_alert_handler::*;
use voice_receiver::*;

use config::{Config as ConfigFile, File};
use guard::guard;
use serenity::model::id::GuildId;
use serenity::model::prelude::{ChannelId, Message, Ready, UserId};
use serenity::prelude::{Context, EventHandler, Mentionable, TypeMapKey};
use serenity::{async_trait, Client};
use songbird::driver::DecodeMode;
use songbird::{Config, SerenityInit};
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::os::raw::c_int;
use std::path::Path;
use std::sync::mpsc::{SyncSender, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use voskrust::api::{set_log_level as set_vosk_log_level, Model as VoskModel};

#[macro_use]
extern crate log;

fn is_sub<T: PartialEq>(first: &Vec<T>, second: &Vec<T>) -> bool {
    if second.is_empty() {
        return false;
    }
    let mut index: usize = 0;
    for element in first {
        if &second[index] == element {
            index += 1;
        } else {
            index = 0;
        }
        if second.len() == index {
            return true;
        }
    }
    false
}

struct VoidSender(SyncSender<()>);

impl TypeMapKey for VoidSender {
    type Value = Self;
}

#[derive(Clone)]
struct VoiceConfig {
    target_words: Vec<String>,
    self_words: Vec<String>,
    aliases: HashMap<String, u64>,
}

impl VoiceConfig {
    fn should_kick(&self, author_user_id: UserId, text: &String) -> Option<UserId> {
        for self_word in &self.self_words {
            if text.contains(self_word) {
                return Some(author_user_id);
            }
        }
        for target_word in &self.target_words {
            if text.contains(target_word) {
                for (name, user_id) in &self.aliases {
                    if text.contains(name) {
                        return Some(UserId(*user_id));
                    }
                }
            }
        }
        None
    }
}

struct Handler {
    recognition_model: VoskModel,
    voice_config: VoiceConfig,
    red_alert_handler: Arc<RedAlertHandler>,
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

        let answer_msg = if is_sub(&message_words, &vec!["отслеживать", "код", "красный"])
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
                listen_for_red_alert(
                    self.red_alert_handler.clone(),
                    &self.recognition_model,
                    &self.voice_config,
                    &ctx,
                    guild_id,
                    possible_channel_id,
                )
                .await
            } else {
                format!("ЧТО ОТСЛЕЖИВАТЬ НАРКОМАН?")
            }
        } else if is_sub(&message_words, &vec!["прекратить", "код", "красный"])
        {
            exit_for_red_alert(&ctx, guild_id).await
        } else if is_sub(&message_words, &vec!["код", "красный"]) {
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
    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

async fn listen_for_red_alert(
    red_alert_handler: Arc<RedAlertHandler>,
    recognition_model: &VoskModel,
    voice_config: &VoiceConfig,
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> String {
    let channel_name = channel_id.mention();

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let (handler_lock, conn_result) = manager.join(guild_id, channel_id).await;

    if conn_result.is_ok() {
        let mut handler = handler_lock.lock().await;

        let voice_receiver = VoiceReceiver::new(handler.deref_mut(), Default::default());

        let (tx, rx) = mpsc::sync_channel::<()>(1);

        let mut data = ctx.data.write().await;
        data.insert::<VoidSender>(VoidSender(tx));

        let red_alert_handler = red_alert_handler.clone();
        let recognition_model = recognition_model.clone();
        let voice_config = voice_config.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let recognizer_signal = Recognizer {
                workers_count: 5,
                model: recognition_model,
                voice_receiver,
            }
            .start();
            let mut session_kicked: HashSet<UserId> = HashSet::new();
            'root: loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
                match rx.try_recv() {
                    Ok(()) | Err(TryRecvError::Disconnected) => {
                        break 'root;
                    }
                    Err(TryRecvError::Empty) => {}
                }

                let recognizer_event = match recognizer_signal.try_recv() {
                    Ok(recognizer_event) => recognizer_event,
                    Err(error) => match error {
                        TryRecvError::Empty => {
                            continue 'root;
                        }
                        TryRecvError::Disconnected => {
                            break 'root;
                        }
                    },
                };

                match recognizer_event.state {
                    RecognizerState::RecognitionResult(user_id, result) => {
                        info!(
                            "[W:{}][UID:{}] Recognition RESULT is {:?}.",
                            recognizer_event.worker_number, user_id.0, result
                        );
                        if session_kicked.contains(&user_id) {
                            info!(
                                "[W:{}][UID:{}] Recognition RESULT skipped, because user already kicked.",
                                recognizer_event.worker_number, user_id.0
                            );
                            continue 'root;
                        }
                        guard!(let Some(kick_user_id) =
                            voice_config.should_kick(user_id, &result.text) else {
                            info!(
                                "[W:{}][UID:{}] Recognition RESULT skipped, because don't have restrictions.",
                                recognizer_event.worker_number, user_id.0
                            );
                            continue 'root;
                        });
                        info!(
                            "[W:{}][UID:{}] Recognition RESULT will be used for kick, because have restrictions.",
                            recognizer_event.worker_number, user_id.0
                        );
                        session_kicked.insert(kick_user_id);
                        let red_alert_handler = red_alert_handler.clone();
                        let ctx = ctx.clone();
                        tokio::spawn(async move {
                            let red_alert_deportations_results = red_alert_handler
                                .handle(&ctx, guild_id, vec![kick_user_id])
                                .await;
                            let red_alert_deportation_result =
                                red_alert_deportations_results.get(&kick_user_id).unwrap();
                            info!(
                                "[W:{}][UID:{}] Recognition RESULT used for kick, status is {:?}.",
                                recognizer_event.worker_number,
                                user_id.0,
                                red_alert_deportation_result
                            );
                        });
                    }
                    RecognizerState::RecognitionStart(user_id) => {
                        info!(
                            "[W:{}][UID:{}] Recognition STARTED.",
                            recognizer_event.worker_number, user_id.0
                        );
                        session_kicked.remove(&user_id);
                    }
                    RecognizerState::RecognitionEnd(user_id) => {
                        info!(
                            "[W:{}][UID:{}] Recognition ENDED.",
                            recognizer_event.worker_number, user_id.0
                        );
                        session_kicked.remove(&user_id);
                    }
                    RecognizerState::Idle => {}
                }
            }
        });

        format!("ОТСЛЕЖИВАЮ КОД КРАСНЫЙ В КАНАЛЕ {channel_name}...")
    } else {
        format!("ОШИБКА СЛЕЖКИ ЗА КАНАЛОМ {channel_name}.")
    }
}

async fn exit_for_red_alert(ctx: &Context, guild_id: GuildId) -> String {
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if manager.remove(guild_id).await.is_err() {
            format!("ПРОИЗОШЛА ОШИБКА!")
        } else {
            let mut data = ctx.data.write().await;
            data.remove::<VoidSender>();
            format!("ПРЕКРАЩАЮ ОТСЛЕЖИВАНИЕ КАНАЛА!")
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

    if let Ok(vosk_log_level) = vosk_log_level {
        set_vosk_log_level(vosk_log_level as c_int);
    }

    let mut client = Client::builder(&token)
        .event_handler(Handler {
            recognition_model: VoskModel::new(vosk_model_path.as_str())
                .expect("Incorrect recognition model!"),
            voice_config: VoiceConfig {
                target_words,
                self_words,
                aliases,
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
