mod recognition;
mod red_alert_handler;
mod voice_receiver;

use recognition::*;
use red_alert_handler::*;
use voice_receiver::*;

use guard::guard;
use serenity::model::prelude::{ChannelId, Guild, Message, Ready, UserId};
use serenity::prelude::{Context, EventHandler, Mentionable, TypeMapKey};
use serenity::{async_trait, Client};
use songbird::driver::DecodeMode;
use songbird::{Config, SerenityInit};
use std::ops::DerefMut;
use std::sync::mpsc;
use std::sync::mpsc::{SyncSender, TryRecvError};
use std::thread;
use voskrust::api::Model;

fn is_sub<T: PartialEq>(first: &Vec<T>, second: &Vec<T>) -> bool {
    if second.len() == 0 {
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

struct Handler {
    recognition_model: Model,
    red_alert_handler: RedAlertHandler,
}

impl Default for Handler {
    fn default() -> Self {
        Self {
            recognition_model: Model::new("vosk-model-small-ru-0.22").unwrap(),
            red_alert_handler: RedAlertHandler,
        }
    }
}

impl Handler {
    async fn listen_for_red_alert(
        &self,
        ctx: &Context,
        guild: &Guild,
        channel_id: ChannelId,
    ) -> String {
        let channel_name = channel_id.mention();
        let guild_id = guild.id;

        let manager = songbird::get(ctx)
            .await
            .expect("Songbird Voice client placed in at initialisation.")
            .clone();

        let (handler_lock, conn_result) = manager.join(guild_id, channel_id).await;

        if let Ok(_) = conn_result {
            let mut handler = handler_lock.lock().await;

            let voice_receiver = VoiceReceiver::default_start_on(handler.deref_mut());

            let recognition_signal = start_recognition(5, &self.recognition_model, &voice_receiver);

            let (tx, rx) = mpsc::sync_channel::<()>(1);
            thread::spawn(move || 'root: loop {
                match rx.try_recv() {
                    Ok(()) | Err(TryRecvError::Disconnected) => {
                        break 'root;
                    }
                    Err(TryRecvError::Empty) => {}
                }

                if let Ok(worker_event) = recognition_signal.recv() {
                    match worker_event {
                        RecognitionWorkerEvent::Event(
                            worker_number,
                            user_id,
                            recognition_event
                        ) => println!(
                            "({}) Event: User ID: {:?}; text: {}; is partial: {}",
                            worker_number,
                            user_id,
                            recognition_event.text,
                            recognition_event.text_type == RecognitionEventType::Partial
                        ),
                        RecognitionWorkerEvent::Idle(worker_number) => println!(
                            "({}) Idle",
                            worker_number
                        ),
                        RecognitionWorkerEvent::Start(worker_number, user_id) => println!(
                            "({}) Start: User ID: {:?}",
                            worker_number,
                            user_id
                        ),
                        RecognitionWorkerEvent::End(worker_number, user_id) => println!(
                            "({}) End: User ID: {:?}",
                            worker_number,
                            user_id
                        )
                    }
                } else {
                    break 'root
                }
            });

            let mut data = ctx.data.write().await;
            data.insert::<VoidSender>(VoidSender(tx));

            format!("ОТСЛЕЖИВАЮ КОД КРАСНЫЙ В КАНАЛЕ {channel_name}...")
        } else {
            format!("ОШИБКА СЛЕЖКИ ЗА КАНАЛОМ {channel_name}.")
        }
    }

    async fn exit_for_red_alert(&self, ctx: &Context, guild: &Guild) -> String {
        let guild_id = guild.id;

        let manager = songbird::get(ctx)
            .await
            .expect("Songbird Voice client placed in at initialisation.")
            .clone();

        let has_handler = manager.get(guild_id).is_some();

        if has_handler {
            if let Err(_) = manager.remove(guild_id).await {
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
        &self,
        ctx: &Context,
        guild: &Guild,
        author_user_id: UserId,
        target_users_ids: Vec<UserId>,
    ) -> String {
        let red_alert_handler = &self.red_alert_handler;

        let red_alert_result = red_alert_handler
            .handle(&ctx, &guild, target_users_ids)
            .await;

        match red_alert_result.len() {
            0 => {
                if let Some(result) = red_alert_handler
                    .handle(&ctx, &guild, vec![author_user_id])
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
                                &ctx,
                                &guild,
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
                        .handle(&ctx, &guild, vec![author_user_id])
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
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        };
        guard!(let Some(guild) = msg.guild(&ctx.cache).await else {
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
                possible_channel_id.map(|n| ChannelId(n))
            };
            if let Some(possible_channel_id) = possible_channel_id {
                self.listen_for_red_alert(&ctx, &guild, possible_channel_id)
                    .await
            } else {
                format!("ЧТО ОТСЛЕЖИВАТЬ НАРКОМАН?")
            }
        } else if is_sub(&message_words, &vec!["прекратить", "код", "красный"])
        {
            self.exit_for_red_alert(&ctx, &guild).await
        } else if is_sub(&message_words, &vec!["код", "красный"]) {
            let author_id = msg.author.id;
            let target_users_ids: Vec<UserId> = msg.mentions.iter().map(|u| u.id).collect();
            self.process_red_alert(&ctx, &guild, author_id, target_users_ids)
                .await
        } else {
            format!("")
        };
        if !answer_msg.is_empty() {
            let _ = msg.channel_id.say(&ctx, answer_msg).await;
        }
    }
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let songbird_config = Config::default().decode_mode(DecodeMode::Decode);

    let mut client = Client::builder(&token)
        .event_handler(Handler::default())
        .register_songbird_from_config(songbird_config)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
