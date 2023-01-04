use super::components::*;
use super::red_alert_handler::*;
use super::red_alert_voice_config::*;
use chrono::{offset, DateTime, Utc};
use serde::{Deserialize, Serialize};
use serenity::model::gateway::Activity;
use serenity::model::id::GuildId;
use serenity::model::prelude::Mention;
use serenity::model::prelude::{ChannelId, OnlineStatus, Ready, UserId};
use serenity::prelude::{Context, Mentionable, SerenityError};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::DerefMut;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::oneshot::{channel, Sender};
use tokio::sync::{Mutex, RwLock};
use voskrust::api::Model as VoskModel;

pub struct RedAlertCommandsHandlerConstructor {
    pub recognition_model: VoskModel,
    pub listening_text: Option<String>,
    pub red_alert_handler: Arc<RedAlertHandler>,
}

enum ActionType {
    VoiceRedAlert {
        author_id: UserId,
        target_id: UserId,
        reason: String,
        is_success: bool,
    },
    TextRedAlert {
        author_id: UserId,
        target_id: UserId,
        is_success: bool,
    },
}

struct ActionInfo {
    time: DateTime<Utc>,
    r#type: ActionType,
}

#[derive(Default)]
struct ActionsHistory(HashMap<GuildId, VecDeque<ActionInfo>>);

impl ActionsHistory {
    fn log_history(&mut self, guild_id: GuildId, action_type: ActionType) {
        let action_info = ActionInfo {
            time: offset::Utc::now(),
            r#type: action_type,
        };
        if let Some(guild_actions_history) = self.0.get_mut(&guild_id) {
            guild_actions_history.push_back(action_info);
            if guild_actions_history.len() > 100 {
                guild_actions_history.pop_front();
            }
        } else {
            self.0.insert(guild_id, VecDeque::from([action_info]));
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct RedAlertGuildsVoiceConfig {
    base: RedAlertVoiceConfig<u64>,
    specific: HashMap<u64, RedAlertVoiceConfig<u64>>,
}

impl RedAlertGuildsVoiceConfig {
    const CONFIG_PATH: &str = "guilds_voice_config.yaml";
    fn read() -> RedAlertGuildsVoiceConfig {
        let config_string =
            std::fs::read_to_string(Self::CONFIG_PATH).expect("Guild voice config read error!");
        let config: RedAlertGuildsVoiceConfig =
            serde_yaml::from_str(&config_string).expect("Guild voice config deserialize error!");
        config
    }
    fn write(&self) {
        let config_string =
            serde_yaml::to_string(self).expect("Guild voice config serialize error!");
        std::fs::write(Self::CONFIG_PATH, config_string).expect("Guild voice config write error!");
    }
    fn get(&self, guild_id: &GuildId) -> &RedAlertVoiceConfig<u64> {
        self.specific.get(&guild_id.0).unwrap_or(&self.base)
    }
}

impl RedAlertCommandsHandlerConstructor {
    pub fn build(self) -> Handler<impl Fn(&str, HelpInfo) -> String> {
        let guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>> =
            Arc::new(Default::default());
        let actions_history: Arc<Mutex<ActionsHistory>> = Arc::new(Default::default());
        let guilds_voice_config = Arc::new(RwLock::new(RedAlertGuildsVoiceConfig::read()));
        Handler {
            help_command: HelpCommandConfig {
                prefix_anchor: "кринж киллер помощь".to_string(),
                output_prefix: None,
                output_format_fn: |prefix_anchor, help_info| {
                    format!(
                        "> **`{}`**\n```{}```\n",
                        help_info
                            .header_suffix
                            .map(|header_suffix| format!("{} {}", prefix_anchor, header_suffix))
                            .unwrap_or(prefix_anchor.to_string()),
                        help_info.description
                    )
                },
            },
            on_ready: Box::new(RedAlertOnReady {
                guilds_voices_receivers: guilds_voices_receivers.clone(),
                actions_history: actions_history.clone(),
                guilds_voice_config: guilds_voice_config.clone(),
                recognition_model: self.recognition_model,
                listening_text: self.listening_text,
                red_alert_handler: self.red_alert_handler.clone(),
                cancel_sender: Arc::new(Mutex::new(None)),
            }),
            commands: vec![
                Box::new(TextRedAlertCommand {
                    red_alert_handler: self.red_alert_handler.clone(),
                    actions_history: actions_history.clone(),
                }),
                Box::new(StartListenRedAlertCommand {
                    guilds_voices_receivers: guilds_voices_receivers.clone(),
                }),
                Box::new(StopListenRedAlertCommand {
                    guilds_voices_receivers: guilds_voices_receivers.clone(),
                }),
                Box::new(ActionsHistoryCommand {
                    actions_history: actions_history.clone(),
                }),
                Box::new(GuildsVoiceConfigCommand {
                    guilds_voice_config: guilds_voice_config.clone(),
                }),
            ],
        }
    }
}

struct RedAlertOnReady {
    guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
    actions_history: Arc<Mutex<ActionsHistory>>,
    guilds_voice_config: Arc<RwLock<RedAlertGuildsVoiceConfig>>,
    recognition_model: VoskModel,
    listening_text: Option<String>,
    red_alert_handler: Arc<RedAlertHandler>,
    cancel_sender: Arc<Mutex<Option<Sender<()>>>>,
}

impl RedAlertOnReady {
    async fn start_recognizer(&self, ctx: &Context) {
        let (tx, mut rx) = channel::<()>();
        let mut cancel_sender = self.cancel_sender.lock().await;
        *cancel_sender = Some(tx);
        drop(cancel_sender);
        let guilds_voices_receivers = self.guilds_voices_receivers.clone();
        let actions_history = self.actions_history.clone();
        let recognition_model = self.recognition_model.clone();
        let guilds_voice_config = self.guilds_voice_config.clone();
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
                        let guild_id = info.guild_id;
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
                        let guilds_voice_config = guilds_voice_config.read().await;
                        let voice_config = guilds_voice_config.get(&info.guild_id);
                        let kick_user_id = voice_config
                            .should_kick(&info.user_id.0, &result.text)
                            .map(|v| UserId(*v));
                        drop(guilds_voice_config);
                        let Some(kick_user_id) = kick_user_id else {
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
                        let actions_history = actions_history.clone();
                        let red_alert_handler = red_alert_handler.clone();
                        let ctx = ctx.clone();
                        tokio::spawn(async move {
                            let guild_id = info.guild_id;
                            let deportation_result = red_alert_handler
                                .single(&ctx, &guild_id, &kick_user_id)
                                .await;
                            info!(
                                "{} Recognition RESULT used for kick, status is {:?}.",
                                log_prefix, deportation_result
                            );
                            actions_history.lock().await.log_history(
                                guild_id,
                                ActionType::VoiceRedAlert {
                                    author_id: info.user_id,
                                    target_id: kick_user_id,
                                    reason: result.text,
                                    is_success: deportation_result.is_deported(),
                                },
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
impl OnReady for RedAlertOnReady {
    async fn process(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
        let activity = self.listening_text.as_ref().map(|t| Activity::listening(t));
        ctx.set_presence(activity, OnlineStatus::Online).await;
        self.start_recognizer(&ctx).await;
    }
}

struct StartListenRedAlertCommand {
    guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
}

enum StartListenError {
    SongbirdMissing,
    ConnectingError,
}

async fn start_listen(
    guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
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
impl Command for StartListenRedAlertCommand {
    fn prefix_anchor(&self) -> &str {
        "слушать код красный"
    }
    fn help_info<'a>(&'a self) -> Option<HelpInfo<'a>> {
        Some(HelpInfo {
            header_suffix: Some("{ID или упоминание канала}"),
            description:
                "Начать слушать выбранный голосовой канал на запрещенные и направленные фразы.",
        })
    }
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let channel_id: Option<ChannelId> = params
            .args
            .first()
            .map(|a| match Mention::from_str(a) {
                Ok(mention) => match mention {
                    Mention::Channel(channel_id) => Some(channel_id),
                    Mention::Role(_) | Mention::User(_) | Mention::Emoji(_, _) => None,
                },
                Err(_) => a.parse::<u64>().ok().map(ChannelId),
            })
            .flatten();
        let answer_msg = if let Some(channel_id) = channel_id {
            let channel_name = channel_id.mention();
            match start_listen(self.guilds_voices_receivers.clone(), &ctx, guild_id, channel_id)
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

struct StopListenRedAlertCommand {
    guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
}

enum StopListenError {
    SongbirdMissing,
    DisconnectingError,
    NoListeners,
}

async fn stop_listen(
    guilds_voices_receivers: Arc<RwLock<HashMap<GuildId, VoiceReceiver>>>,
    ctx: &Context,
    guild_id: GuildId,
) -> Result<(), StopListenError> {
    let Some(manager) = songbird::get(ctx).await else {
        return Err(StopListenError::SongbirdMissing);
    };
    if !manager.get(guild_id).is_some() {
        return Err(StopListenError::NoListeners);
    }
    if manager.remove(guild_id).await.is_err() {
        return Err(StopListenError::DisconnectingError);
    }
    let mut guilds_voices_receivers = guilds_voices_receivers.write().await;
    guilds_voices_receivers.remove(&guild_id);
    Ok(())
}

#[async_trait]
impl Command for StopListenRedAlertCommand {
    fn prefix_anchor(&self) -> &str {
        "прекратить слушать код красный"
    }
    fn help_info<'a>(&'a self) -> Option<HelpInfo<'a>> {
        Some(HelpInfo {
            header_suffix: None,
            description:
                "Прекратить слушать голосовой канал в котором находится КРИНЖ КИЛЛЕР на запрещенные и направленные фразы.",
        })
    }
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let answer_msg =
            match stop_listen(self.guilds_voices_receivers.clone(), &ctx, guild_id).await {
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

struct TextRedAlertCommand {
    actions_history: Arc<Mutex<ActionsHistory>>,
    red_alert_handler: Arc<RedAlertHandler>,
}

enum CommonRedAlertResult {
    Empty {
        auto_self_kick_result: RedAlertDeportationResult,
    },
    SingleSuccess {
        is_self_kick: bool,
    },
    SingleNotFound {
        auto_self_kick_result: Option<RedAlertDeportationResult>,
    },
    SingleError {
        error: SerenityError,
        is_self_kick_try: bool,
    },
    Mass {
        results: Vec<RedAlertDeportationResult>,
        auto_self_kick_result: Option<RedAlertDeportationResult>,
    },
}

async fn common_red_alert(
    red_alert_handler: Arc<RedAlertHandler>,
    ctx: &Context,
    guild_id: &GuildId,
    author_user_id: &UserId,
    target_users_ids: &Vec<UserId>,
) -> CommonRedAlertResult {
    let mut red_alert_results = red_alert_handler
        .multiple(ctx, guild_id, target_users_ids)
        .await;
    match red_alert_results.len() {
        0 => CommonRedAlertResult::Empty {
            auto_self_kick_result: red_alert_handler
                .single(ctx, guild_id, author_user_id)
                .await,
        },
        1 => {
            let is_self_kick = *author_user_id == target_users_ids[0];
            match red_alert_results.remove(0) {
                RedAlertDeportationResult::Deported => {
                    CommonRedAlertResult::SingleSuccess { is_self_kick }
                }
                RedAlertDeportationResult::NotFound => CommonRedAlertResult::SingleNotFound {
                    auto_self_kick_result: if !is_self_kick {
                        Some(
                            red_alert_handler
                                .single(ctx, guild_id, author_user_id)
                                .await,
                        )
                    } else {
                        None
                    },
                },
                RedAlertDeportationResult::Error(error) => CommonRedAlertResult::SingleError {
                    error,
                    is_self_kick_try: is_self_kick,
                },
            }
        }
        _ => {
            let mut any_founded = false;
            for red_alert_result in &red_alert_results {
                if red_alert_result.is_not_found() {
                    continue;
                }
                any_founded = true;
                break;
            }
            CommonRedAlertResult::Mass {
                results: red_alert_results,
                auto_self_kick_result: if !any_founded && !target_users_ids.contains(author_user_id)
                {
                    Some(
                        red_alert_handler
                            .single(ctx, guild_id, author_user_id)
                            .await,
                    )
                } else {
                    None
                },
            }
        }
    }
}

#[async_trait]
impl Command for TextRedAlertCommand {
    fn prefix_anchor(&self) -> &str {
        "код красный"
    }
    fn help_info<'a>(&'a self) -> Option<HelpInfo<'a>> {
        Some(HelpInfo {
            header_suffix: Some("{ID или упоминание пользователя}*"),
            description:
                "* - может быть несколько (через пробел).\nКикает выбранного пользователя из голосового канала если он в нем находится, иначе, кикает исполнителя команды.",
        })
    }
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
                    Mention::User(user_id) => Some(user_id),
                    Mention::Channel(_) | Mention::Role(_) | Mention::Emoji(_, _) => None,
                },
                Err(_) => a.parse::<u64>().ok().map(UserId),
            })
            .collect();
        let answer_msg = match common_red_alert(
            self.red_alert_handler.clone(),
            &ctx,
            &guild_id,
            &author_id,
            &target_users_ids,
        )
        .await
        {
            CommonRedAlertResult::Empty {
                auto_self_kick_result,
            } => {
                self.actions_history.lock().await.log_history(
                    guild_id,
                    ActionType::TextRedAlert {
                        author_id,
                        target_id: author_id,
                        is_success: auto_self_kick_result.is_deported(),
                    },
                );
                match auto_self_kick_result {
                    RedAlertDeportationResult::Deported => format!("ВИЖУ ТЫ ЗАБЫЛ УКАЗАТЬ ЦЕЛЬ ДЛЯ КРАСНОГО КОДА, НИЧЕГО... ШМАЛЬНЕМ В ТЕБЯ! (ИСПОЛЬЗУЙ ТЕГИ) ПРИНЯТО К ИСПОЛНЕНИЮ!"),
                    RedAlertDeportationResult::NotFound => format!(":face_with_monocle: ПОЛЬЗУЙСЯ ТЕГАМИ, И ЛУЧШЕ НЕ ЗАХОДИ В КАНАЛ, А ТО КИКНУ С ТАКИМИ ПРИКОЛАМИ! Пшшшш..."),
                    RedAlertDeportationResult::Error(_) => format!("СЛОМАЛСЯ ПОКА ПЫТАЛСЯ ТЕБЯ КИКНУТЬ ЧТО НЕПРАВИЛЬНОЕ ИСПОЛЬЗОВАНИЕ, КАК ВСЕГДА КОД ГОВНА! ОТМЕНА! Пшшшш...")
                }
            }
            CommonRedAlertResult::SingleSuccess { is_self_kick } => {
                self.actions_history.lock().await.log_history(
                    guild_id,
                    ActionType::TextRedAlert {
                        author_id,
                        target_id: target_users_ids[0],
                        is_success: true,
                    },
                );
                if is_self_kick {
                    format!("КОД КРАСНЫЙ ПОДТВЕРЖДЕН! САМОВЫПИЛ ДЕЛО ДОСТОЙНОЕ!!! 0)00))00")
                } else {
                    let user_name = target_users_ids[0].mention();
                    format!("КОД КРАСНЫЙ ПОДТВЕРЖДЕН! АНТИКРИНЖ ОРУЖИЕ ИСПОЛЬЗОВАНО ПРОТИВ {user_name}!!! 0)00))00")
                }
            }
            CommonRedAlertResult::SingleNotFound {
                auto_self_kick_result,
            } => {
                let mut actions_history = self.actions_history.lock().await;
                actions_history.log_history(
                    guild_id,
                    ActionType::TextRedAlert {
                        author_id,
                        target_id: target_users_ids[0],
                        is_success: false,
                    },
                );
                if let Some(self_kick_result) = auto_self_kick_result {
                    actions_history.log_history(
                        guild_id,
                        ActionType::TextRedAlert {
                            author_id,
                            target_id: author_id,
                            is_success: self_kick_result.is_deported(),
                        },
                    );
                    match self_kick_result {
                        RedAlertDeportationResult::Deported => format!("В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЗНАЧИТ У ТЕБЯ БЕДЫ С БОШКОЙ, КОД КРАСНЫЙ НА ТЕБЯ!"),
                        RedAlertDeportationResult::NotFound => format!("ДОФИГА УМНЫЙ ВИЖУ? В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЖАЛЬ ТЕБЯ В КАНАЛЕ НЕТУ, ТАК БЫ ТЕБЯ ШМАЛЬНУЛ КОДОМ КРАСНЫМ! ОТМЕНА! Пшшшш..."),
                        RedAlertDeportationResult::Error(_) => format!("ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ТО ЧТО ТЫ ПЫТАЛСЯ КИКНУТЬ ТОГО КОГО НЕТ, НО Я СЛОМАЛСЯ! Пшшшш...")
                    }
                } else {
                    format!("СУИЦИД ЭТО ПЛОХО ТАК ЧТО НЕТ))) (У меня просто не получилось)")
                }
            }
            CommonRedAlertResult::SingleError {
                error: _,
                is_self_kick_try,
            } => {
                self.actions_history.lock().await.log_history(
                    guild_id,
                    ActionType::TextRedAlert {
                        author_id,
                        target_id: target_users_ids[0],
                        is_success: false,
                    },
                );
                if is_self_kick_try {
                    format!("АУЧ, МАСЛИНУ ПОЙМАЛ, НЕ СМОГ ОРГАНИЗОВАТЬ ТЕБЕ СУИЦИД0))")
                } else {
                    format!("АУЧ, МАСЛИНУ ПОЙМАЛ, ОШИБКА В СИСТЕМЕё0))")
                }
            }
            CommonRedAlertResult::Mass {
                results,
                auto_self_kick_result,
            } => {
                let mut actions_history = self.actions_history.lock().await;
                for result_index in 0..results.len() {
                    actions_history.log_history(
                        guild_id,
                        ActionType::TextRedAlert {
                            author_id,
                            target_id: target_users_ids[result_index],
                            is_success: results[result_index].is_deported(),
                        },
                    );
                }
                if let Some(auto_self_kick_result) = auto_self_kick_result {
                    actions_history.log_history(
                        guild_id,
                        ActionType::TextRedAlert {
                            author_id,
                            target_id: author_id,
                            is_success: auto_self_kick_result.is_deported(),
                        },
                    );
                    match auto_self_kick_result {
                        RedAlertDeportationResult::Deported => format!("МАССОВЫЙ КОД КРАСНЫЙ ШТУКА ОПАСНАЯ, ТАК КАК ПО РАЗНЫМ ПРИЧИНАМ Я НИКОГО НЕ КИКНУЛ, КИКНУ ТЕБЯ )В)В)))0"),
                        RedAlertDeportationResult::NotFound => format!("ЖАЛЬ ТЕБЯ НЕ МОГУ ПРШИТЬ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ! ОТМЕНА Пшшшш..."),
                        RedAlertDeportationResult::Error(_) => format!("ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ, НО САМ ОБО****СЯ! Пшшшш...")
                    }
                } else {
                    let mut result_strings = Vec::new();
                    for index in 0..results.len() {
                        let deport_status = &results[index];
                        let user_name = target_users_ids[index].mention();
                        let deport_status = match deport_status {
                            RedAlertDeportationResult::Deported => "ИСПОЛНЕНО",
                            RedAlertDeportationResult::NotFound => "НЕ В КАНАЛЕ",
                            RedAlertDeportationResult::Error(_) => "ОШИБКА (ПРОЧНЫЙ СУ*А)",
                        };
                        let record_number = index + 1;
                        result_strings.push(format!(
                            " {record_number}. {user_name} СТАТУС: {deport_status}."
                        ))
                    }
                    let result_string = result_strings.join("\n");
                    format!("ОУ, МАССОВЫЙ КОД КРАСНЫЙ? СТАТУС ВЫКОСА КРИНЖОВИКОВ:\n{result_string}")
                }
            }
        };
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}

struct ActionsHistoryCommand {
    actions_history: Arc<Mutex<ActionsHistory>>,
}

#[async_trait]
impl Command for ActionsHistoryCommand {
    fn prefix_anchor(&self) -> &str {
        "код красный история"
    }
    fn help_info<'a>(&'a self) -> Option<HelpInfo<'a>> {
        Some(HelpInfo {
            header_suffix: None,
            description: "Выводит историю всех наказаний которые исполнил КРИНЖ КИЛЛЕР.",
        })
    }
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let mut actions_history = self.actions_history.lock().await;
        let answer_msg = if let Some(guild_history) = (*actions_history).0.get(&guild_id) {
            let mut result_strings = vec![];
            for action_info_index in 0..guild_history.len() {
                let time = &guild_history[action_info_index].time.to_rfc2822();
                let info_string = match &guild_history[action_info_index].r#type {
                    ActionType::VoiceRedAlert {
                        author_id,
                        target_id,
                        reason,
                        is_success,
                    } => {
                        let autor = author_id.mention();
                        let target = target_id.mention();
                        if author_id == target_id {
                            let status = if *is_success {
                                "САМОВЫПИЛИЛСЯ"
                            } else {
                                "ПОПЫТАЛСЯ САМОВЫПИЛИТЬСЯ"
                            };
                            format!("КРИНЖОВИК {target} {status} ФРАЗОЙ \"{reason}\".")
                        } else {
                            let status = if *is_success {
                                "КИКНУТ"
                            } else {
                                "ПОЧТИ... КИКНУТ"
                            };
                            format!("КРИНЖОВИК {target} {status} ГОЛОСОМ МИРОТВОРЦA {autor} ПРИ ПОМОЩИ ФРАЗЫ \"{reason}\".")
                        }
                    }
                    ActionType::TextRedAlert {
                        author_id,
                        target_id,
                        is_success,
                    } => {
                        let author = author_id.mention();
                        let target = target_id.mention();
                        if author_id == target_id {
                            let status = if *is_success {
                                "САМОВЫПИЛИЛСЯ"
                            } else {
                                "ПОПЫТАЛСЯ САМОВЫПИЛИТЬСЯ"
                            };
                            format!("КРИНЖОВИК {target} {status} КОМАНДОЙ")
                        } else {
                            let status = if *is_success {
                                "КИКНУТ"
                            } else {
                                "ПОЧТИ... КИКНУТ"
                            };
                            format!("КРИНЖОВИК {target} {status} КОМАНДОЙ МИРОТВОРЦA {author}")
                        }
                    }
                };
                let record_number = action_info_index + 1;
                result_strings.push(format!(" {record_number}. [ВРЕМЯ: {time}] {info_string}."));
            }
            let result_string = result_strings.join("\n");
            format!("ИСТОРИЯ ВЫКОСА КРИНЖОВИКОВ:\n{result_string}")
        } else {
            format!("ПОКА ЕЩЕ НИКОГО НЕ УШАТАЛ НА ЭТОМ СЕРВЕР)!1!))")
        };
        (*actions_history).0.remove(&guild_id);
        drop(actions_history);
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}

struct GuildsVoiceConfigCommand {
    guilds_voice_config: Arc<RwLock<RedAlertGuildsVoiceConfig>>,
}

impl GuildsVoiceConfigCommand {
    fn process_self_words(
        guild_voice_config: &mut RedAlertVoiceConfig<u64>,
        args: Vec<String>,
    ) -> String {
        let word = args.join(" ");
        if let Some(index) = guild_voice_config
            .self_words
            .iter()
            .position(|w| w == &word)
        {
            guild_voice_config.self_words.remove(index);
            format!("ЗАПРЕТНАЯ ФРАЗА УДАЛЕНА!")
        } else {
            guild_voice_config.self_words.push(word);
            format!("ЗАПРЕТНАЯ ФРАЗА ДОБАВЛЕНА!")
        }
    }
    fn process_target_words(
        guild_voice_config: &mut RedAlertVoiceConfig<u64>,
        args: Vec<String>,
    ) -> String {
        let word = args.join(" ");
        if let Some(index) = guild_voice_config
            .target_words
            .iter()
            .position(|w| w == &word)
        {
            guild_voice_config.target_words.remove(index);
            format!("ВЫГОНЯЮЩАЯ ФРАЗА УДАЛЕНА!")
        } else {
            guild_voice_config.target_words.push(word);
            format!("ВЫГОНЯЮЩАЯ ФРАЗА ДОБАВЛЕНА!")
        }
    }
    fn process_aliases(
        guild_voice_config: &mut RedAlertVoiceConfig<u64>,
        mut args: Vec<String>,
    ) -> String {
        if !(args.len() > 1) {
            return format!("МАЛО ПАРАМЕТРОВ!");
        }
        let user_id_string = args.remove(args.len() - 1);
        let Some(user_id) = (match Mention::from_str(&*user_id_string) {
            Ok(mention) => match mention {
                Mention::User(user_id) => Some(user_id),
                Mention::Channel(_) | Mention::Role(_) | Mention::Emoji(_, _) => None,
            },
            Err(_) => user_id_string.parse::<u64>().ok().map(UserId),
        }) else {
            return format!("НЕВЕРНЫЙ ПОЛЬЗОВАТЕЛЬ!")
        };
        let word = args.join(" ");
        let Some(saved_user_id) = guild_voice_config.aliases.remove(&word) else {
            guild_voice_config.aliases.insert(word, user_id.0);
            return format!("ДОБАВЛЕН ПСЕВДОНИМ ДЛЯ {}!", user_id.mention())
        };
        if saved_user_id == user_id.0 {
            format!("УДАЛЕН ПСЕВДОНИМ ДЛЯ {}!", user_id.mention())
        } else {
            guild_voice_config.aliases.insert(word, user_id.0);
            format!("ЗАМЕНЕН ПСЕВДОНИМ ДЛЯ {}!", user_id.mention())
        }
    }
    fn process_similarity_threshold(
        guild_voice_config: &mut RedAlertVoiceConfig<u64>,
        mut args: Vec<String>,
    ) -> String {
        if !(args.len() > 0) {
            return format!("НЕ УКАЗАНА ПОГРЕШНОСТЬ!");
        }
        let similarity_threshold_string = args.remove(0);
        let Ok(similarity_threshold) = similarity_threshold_string.parse::<f32>() else {
            return format!("НЕПРАВИЛЬНЫЙ ФОРМАТ ПОГРЕШНОСТИ!")
        };
        let similarity_threshold = similarity_threshold.max(0.0).min(1.0);
        guild_voice_config.similarity_threshold = similarity_threshold;
        format!(
            "ПОГРЕШНОСТЬ ОБНОВЛЕНА НА ЗНАЧЕНИЕ: {}!",
            similarity_threshold
        )
    }
    fn format(guild_voice_config: &RedAlertVoiceConfig<u64>) -> String {
        format!(
            "Запретные:\n{}Выгоняющие:\n{}Псевдонимы:\n{}Погрешность: {}",
            guild_voice_config
                .self_words
                .iter()
                .map(|a| format!(" - {}\n", a))
                .collect::<String>(),
            guild_voice_config
                .target_words
                .iter()
                .map(|a| format!(" - {}\n", a))
                .collect::<String>(),
            guild_voice_config
                .aliases
                .iter()
                .map(|a| format!(" - {}: {}\n", a.0, UserId(*a.1).mention()))
                .collect::<String>(),
            guild_voice_config.similarity_threshold,
        )
    }
}

#[async_trait]
impl Command for GuildsVoiceConfigCommand {
    fn prefix_anchor(&self) -> &str {
        "код красный фраза"
    }
    fn help_info<'a>(&'a self) -> Option<HelpInfo<'a>> {
        Some(HelpInfo {
            header_suffix: Some("[запретная/выгоняющая/псевдоним/погрешность/список]"),
            description:
                "[запретная] {фраза} - добавляет/удаляет фразу при призношении которой пользователь будет исключен.\n[выгоняющая] {фраза} - добавляет/удаляет фразу при призношении которой пользователь может исключить другого пользователя.\n[псевдоним] {фраза} {ID или упоминание пользователя} - добавляет/удаляет псевдоним для пользователя который можно использовать в распознавателе речи.\n[погрешность] {0.0 - 1.0} - устанавливает погрешность разпознавания речи.\n[список] - список всех фраз.",
        })
    }
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let mut guilds_voice_config = self.guilds_voice_config.write().await;
        let mut guild_voice_config = {
            if let Some(specific) = guilds_voice_config.specific.remove(&guild_id.0) {
                specific
            } else {
                let base = guilds_voice_config.base.clone();
                base
            }
        };
        let mut args = params.args.to_vec();
        let answer_msg = if args.len() > 0 {
            match &*args.remove(0) {
                "запретная" => Self::process_self_words(&mut guild_voice_config, args),
                "выгоняющая" => Self::process_target_words(&mut guild_voice_config, args),
                "псевдоним" => Self::process_aliases(&mut guild_voice_config, args),
                "погрешность" => {
                    Self::process_similarity_threshold(&mut guild_voice_config, args)
                }
                "список" => Self::format(&guild_voice_config),
                _ => format!("НЕТУ ТАКОГО ДЕЙСТВИЯ!"),
            }
        } else {
            format!("НЕ УКАЗАНО ДЕЙСТВИЕ!")
        };
        guilds_voice_config
            .specific
            .insert(guild_id.0, guild_voice_config);
        guilds_voice_config.write();
        drop(guilds_voice_config);
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}
