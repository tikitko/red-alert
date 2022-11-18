use crate::*;
use async_trait::async_trait;
use serenity::model::prelude::Mention;
use std::str::FromStr;

#[derive(Clone)]
pub struct RedAlertCommandsConfig {
    pub listening_text: Option<String>,
    pub voice: VoiceConfig,
}

pub struct CommandsHandlerConstructor {
    pub recognition_model: VoskModel,
    pub config: RedAlertCommandsConfig,
    pub red_alert_handler: Arc<RedAlertHandler>,
}

impl Into<Handler> for CommandsHandlerConstructor {
    fn into(self) -> Handler {
        let guilds_voices_receivers: Arc<tokio::sync::RwLock<HashMap<GuildId, VoiceReceiver>>> =
            Arc::new(Default::default());
        let mut handler = Handler::new(OnReadyRedAlert {
            guilds_voices_receivers: guilds_voices_receivers.clone(),
            recognition_model: self.recognition_model,
            config: self.config,
            red_alert_handler: self.red_alert_handler.clone(),
            cancel_sender: Arc::new(tokio::sync::Mutex::new(None)),
        });
        handler.insert_command(
            "код красный".to_string(),
            TextCommandRedAlert {
                red_alert_handler: self.red_alert_handler.clone(),
            },
        );
        handler.insert_command(
            "отсеживать код красный".to_string(),
            StartListenCommandRedAlert {
                guilds_voices_receivers: guilds_voices_receivers.clone(),
            },
        );
        handler.insert_command(
            "прекратить код красный".to_string(),
            StopListenCommandRedAlert {
                guilds_voices_receivers: guilds_voices_receivers.clone(),
            },
        );
        handler
    }
}

struct OnReadyRedAlert {
    guilds_voices_receivers: Arc<tokio::sync::RwLock<HashMap<GuildId, VoiceReceiver>>>,
    recognition_model: VoskModel,
    config: RedAlertCommandsConfig,
    red_alert_handler: Arc<RedAlertHandler>,
    #[allow(dead_code)]
    cancel_sender: Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl OnReadyRedAlert {
    async fn start_recognizer(&self, ctx: &Context) {
        let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
        {
            let mut cancel_sender = self.cancel_sender.lock().await;
            *cancel_sender = Some(tx);
        }

        let guilds_voices_receivers = self.guilds_voices_receivers.clone();
        let recognition_model = self.recognition_model.clone();
        let voice_config = self.config.voice.clone();
        let red_alert_handler = self.red_alert_handler.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let mut recognizer_signal = Recognizer {
                model: recognition_model,
                voices_queue: GuildsVoicesReceivers(guilds_voices_receivers),
            }
            .start();
            let mut session_kicked: HashSet<UserId> = HashSet::new();
            loop {
                let Some(recognizer_state) = tokio::select! {
                    recognizer_state = recognizer_signal.recv() => recognizer_state,
                    _ = &mut rx => None,
                } else {
                    break;
                };

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
                        let Some(kick_user_id) = voice_config.should_kick(
                            info.user_id,
                            &result.text
                        ) else {
                            continue;
                        };
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
                            let guild_id = info.inner.guild_id;
                            let deportation_result = red_alert_handler
                                .single(&ctx, &guild_id, &kick_user_id)
                                .await;
                            info!(
                                "{} Recognition RESULT used for kick, status is {:?}.",
                                log_prefix, deportation_result
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
impl OnReady for OnReadyRedAlert {
    async fn process(&self, ctx: Context, ready: Ready) {
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

struct StartListenCommandRedAlert {
    guilds_voices_receivers: Arc<tokio::sync::RwLock<HashMap<GuildId, VoiceReceiver>>>,
}

enum StartListenError {
    SongbirdMissing,
    ConnectingError,
}

async fn start_listen_red_alert(
    guilds_voices_receivers: Arc<tokio::sync::RwLock<HashMap<GuildId, VoiceReceiver>>>,
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<(), StartListenError> {
    let Some(manager) = songbird::get(ctx).await else {
        return Err(StartListenError::SongbirdMissing);
    };

    let (handler_lock, connection_result) = manager.join(guild_id, channel_id).await;

    if !connection_result.is_ok() {
        return Err(StartListenError::ConnectingError);
    }
    let mut handler = handler_lock.lock().await;

    let voice_receiver = VoiceReceiver::with_configuration(Default::default());
    voice_receiver.subscribe(handler.deref_mut());

    let mut guilds_voices_receivers = guilds_voices_receivers.write().await;
    guilds_voices_receivers.insert(guild_id, voice_receiver);

    Ok(())
}

#[async_trait]
impl Command for StartListenCommandRedAlert {
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let channel_id: Option<ChannelId> = params
            .args
            .first()
            .map(|a| a.parse::<u64>().ok())
            .flatten()
            .map(ChannelId);
        let answer_msg = if let Some(channel_id) = channel_id {
            let channel_name = channel_id.mention();
            match start_listen_red_alert(self.guilds_voices_receivers.clone(), &ctx, guild_id, channel_id)
                .await
            {
                Ok(_) => {
                    format!("ОТСЛЕЖИВАЮ КОД КРАСНЫЙ В КАНАЛЕ {channel_name}...")
                },
                Err(error) => match error {
                    StartListenError::ConnectingError => format!(
                        "ОШИБКА СЛЕЖКИ ЗА КАНАЛОМ {channel_name}. НЕ ПОЛУЧАЕТСЯ ВОЙТИ В КАНАЛ..."
                    ),
                    StartListenError::SongbirdMissing => format!(
                        "ОШИБКА СЛЕЖКИ ЗА КАНАЛОМ {channel_name}. ЗВУКОВАЯ БИБЛИОТЕКА ОТСУТСТВУЕТ..."
                    ),
                },
            }
        } else {
            format!("ЧТО ОТСЛЕЖИВАТЬ НАРКОМАН?")
        };
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}

struct StopListenCommandRedAlert {
    guilds_voices_receivers: Arc<tokio::sync::RwLock<HashMap<GuildId, VoiceReceiver>>>,
}

enum StopListenError {
    SongbirdMissing,
    DisconnectingError,
    NoListeners,
}

async fn stop_listen_red_alert(
    guilds_voices_receivers: Arc<tokio::sync::RwLock<HashMap<GuildId, VoiceReceiver>>>,
    ctx: &Context,
    guild_id: GuildId,
) -> Result<(), StopListenError> {
    let Some(manager) = songbird::get(ctx).await else {
        return Err(StopListenError::SongbirdMissing);
    };
    if manager.get(guild_id).is_some() {
        if manager.remove(guild_id).await.is_err() {
            return Err(StopListenError::DisconnectingError);
        } else {
            let mut guilds_voices_receivers = guilds_voices_receivers.write().await;
            guilds_voices_receivers.remove(&guild_id);
            return Ok(());
        }
    } else {
        return Err(StopListenError::NoListeners);
    }
}

#[async_trait]
impl Command for StopListenCommandRedAlert {
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let answer_msg =
            match stop_listen_red_alert(self.guilds_voices_receivers.clone(), &ctx, guild_id).await
            {
                Ok(_) => {
                    format!("ПРЕКРАЩАЮ ОТСЛЕЖИВАНИЕ КАНАЛА!")
                }
                Err(error) => match error {
                    StopListenError::DisconnectingError => {
                        format!("ПРОИЗОШЛА ОШИБКА! НЕ ПОЛУЧАЕТСЯ ОТКЛЮЧИТЬСЯ...")
                    }
                    StopListenError::SongbirdMissing => {
                        format!("ЗВУКОВАЯ БИБЛИОТЕКА ОТСУТСТВУЕТ...")
                    }
                    StopListenError::NoListeners => format!("НЕ ОТСЛЕЖИВАЮ КАНАЛЫ!"),
                },
            };
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}

struct TextCommandRedAlert {
    red_alert_handler: Arc<RedAlertHandler>,
}

async fn process_red_alert(
    red_alert_handler: Arc<RedAlertHandler>,
    ctx: &Context,
    guild_id: GuildId,
    author_user_id: UserId,
    target_users_ids: Vec<UserId>,
) -> String {
    let red_alert_result = red_alert_handler
        .multiple(ctx, &guild_id, &target_users_ids)
        .await;
    match red_alert_result.len() {
        0 => match red_alert_handler
            .single(ctx, &guild_id, &author_user_id)
            .await {
            RedAlertDeportationResult::Deported => format!("ВИЖУ ТЫ ЗАБЫЛ УКАЗАТЬ ЦЕЛЬ ДЛЯ КРАСНОГО КОДА, НИЧЕГО... ШМАЛЬНЕМ В ТЕБЯ! (ИСПОЛЬЗУЙ ТЕГИ) ПРИНЯТО К ИСПОЛНЕНИЮ!"),
            RedAlertDeportationResult::NotFound => format!(":face_with_monocle: ПОЛЬЗУЙСЯ ТЕГАМИ, И ЛУЧШЕ НЕ ЗАХОДИ В КАНАЛ, А ТО КИКНУ С ТАКИМИ ПРИКОЛАМИ! Пшшшш..."),
            RedAlertDeportationResult::Error(_) => format!("СЛОМАЛСЯ ПОКА ПЫТАЛСЯ ТЕБЯ КИКНУТЬ ЧТО НЕПРАВИЛЬНОЕ ИСПОЛЬЗОВАНИЕ, КАК ВСЕГДА КОД ГОВНА! ОТМЕНА! Пшшшш...")
        }
        1 => match &red_alert_result[0] {
            RedAlertDeportationResult::Deported => {
                let user_name = target_users_ids[0].mention();
                format!("КОД КРАСНЫЙ ПОДТВЕРЖДЕН! АНТИКРИНЖ ОРУЖИЕ ИСПОЛЬЗОВАНО ПРОТИВ {user_name}!!! 0)00))00")
            },
            RedAlertDeportationResult::NotFound => match red_alert_handler
                .single(ctx, &guild_id, &author_user_id)
                .await {
                RedAlertDeportationResult::Deported => format!("В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЗНАЧИТ У ТЕБЯ БЕДЫ С БОШКОЙ, КОД КРАСНЫЙ НА ТЕБЯ!"),
                RedAlertDeportationResult::NotFound => format!("ДОФИГА УМНЫЙ ВИЖУ? В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЖАЛЬ ТЕБЯ В КАНАЛЕ НЕТУ, ТАК БЫ ТЕБЯ ШМАЛЬНУЛ КОДОМ КРАСНЫМ! ОТМЕНА! Пшшшш..."),
                RedAlertDeportationResult::Error(_) => format!("ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ТО ЧТО ТЫ ПЫТАЛСЯ КИКНУТЬ ТОГО КОГО НЕТ, НО Я СЛОМАЛСЯ! Пшшшш...")
            }
            RedAlertDeportationResult::Error(_) => {
                format!("АУЧ, МАСЛИНУ ПОЙМАЛ, ОШИБКА В СИСТЕМЕё0))")
            }
        }
        count => {
            let mut some_kicked = false;
            let mut result_strings = Vec::new();
            for index in 0..count {
                let deport_status = &red_alert_result[index];
                let user_name = target_users_ids[index].mention();
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
                match red_alert_handler
                    .single(ctx, &guild_id, &author_user_id)
                    .await {
                    RedAlertDeportationResult::Deported => format!("МАССОВЫЙ КОД КРАСНЫЙ ШТУКА ОПАСНАЯ, ТАК КАК ПО РАЗНЫМ ПРИЧИНАМ Я НИКОГО НЕ КИКНУЛ, КИКНУ ТЕБЯ )В)В)))0"),
                    RedAlertDeportationResult::NotFound => format!("ЖАЛЬ ТЕБЯ НЕ МОГУ ПРШИТЬ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ! ОТМЕНА Пшшшш..."),
                    RedAlertDeportationResult::Error(_) => format!("ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ, НО САМ ОБО****СЯ! Пшшшш...")
                }
            }
        }
    }
}

#[async_trait]
impl Command for TextCommandRedAlert {
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let author_id = params.author.id;
        let target_users_ids: Vec<UserId> = params
            .args
            .iter()
            .filter_map(|a| match Mention::from_str(a) {
                Ok(mention) => match mention {
                    Mention::Channel(_) => None,
                    Mention::Role(_) => None,
                    Mention::User(user_id) => Some(user_id),
                    Mention::Emoji(_, _) => None,
                },
                Err(_) => None,
            })
            .collect();
        let answer_msg = process_red_alert(
            self.red_alert_handler.clone(),
            &ctx,
            guild_id,
            author_id,
            target_users_ids,
        )
        .await;
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}
