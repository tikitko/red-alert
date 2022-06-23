mod red_alert_handler;

use red_alert_handler::*;
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::*;

struct Handler {
    red_alert_handler: red_alert_handler::RedAlertHandler,
}

impl Default for Handler {
    fn default() -> Self {
        Self {
            red_alert_handler: red_alert_handler::RedAlertHandler::new(
                std::collections::HashSet::from(["код красный ".to_string(), "код к ".to_string()]),
                std::collections::HashSet::from([UserId(224181375912116227)]),
            ),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let red_alert_handler = &self.red_alert_handler;
        async fn user_name(ctx: &Context, user_id: &UserId) -> Option<String> {
            if let Some(cached_user_name) = ctx.cache.user(user_id).await.map(|u| u.name) {
                return Some(cached_user_name);
            }
            if let Ok(received_user_name) = ctx.http.get_user(user_id.0).await.map(|u| u.name) {
                return Some(received_user_name);
            }
            None
        }
        let answer_msg = match red_alert_handler.answer(&ctx, &msg).await {
            RedAlertHandlerAnswer::Empty => format!(""),
            RedAlertHandlerAnswer::NotGuildChat => {
                format!("Инструкцию читай, валенок...")
            }
            RedAlertHandlerAnswer::BlockedUser(user_id) => {
                if let Some(user_name) = user_name(&ctx, &user_id).await {
                    format!("КОРОЛЬ КРИНЖА НЕ ИМЕЕТ ДОСТУПА К ОРУЖИЮ! ОТМЕНА ОПЕРАЦИИ! {user_name} ИДЕТ ДОМОЙ! 0)0000")
                } else {
                    format!("ТЫ ВООБЩЕ КТО ТАКОЙ? ЭНИВЕЙ ТЕБЕ НЕЛЬЗЯ ИСПОЛЬЗОВАТЬ ОРУЖИЕ, ИДИ ПОЧИЛЬ! 0)0000")
                }
            }
            RedAlertHandlerAnswer::DeportationResult(result) => {
                match result.len() {
                    0 => {
                        if let Some(suicide_deport_status) = red_alert_handler
                            .suicide_author_if_possible(&ctx, &msg)
                            .await
                        {
                            match suicide_deport_status {
                            RedAlertDeportationUserResult::Deported => format!("ВИЖУ ТЫ ЗАБЫЛ УКАЗАТЬ ЦЕЛЬ ДЛЯ КРАСНОГО КОДА, НИЧЕГО... ШМАЛЬНЕМ В ТЕБЯ! (ИСПОЛЬЗУЙ ТЕГИ) ПРИНЯТО К ИСПОЛНЕНИЮ!"),
                            RedAlertDeportationUserResult::NotFound => format!(":face_with_monocle: ПОЛЬЗУЙСЯ ТЕГАМИ, И ЛУЧШЕ НЕ ЗАХОДИ В КАНАЛ, А ТО КИКНУ С ТАКИМИ ПРИКОЛАМИ! Пшшшш..."),
                            RedAlertDeportationUserResult::Error(_) => format!("СЛОМАЛСЯ ПОКА ПЫТАЛСЯ ТЕБЯ КИКНУТЬ ЧТО НЕПРАВИЛЬНОЕ ИСПОЛЬЗОВАНИЕ, КАК ВСЕГДА КОД ГОВНА! ОТМЕНА! Пшшшш...")
                        }
                        } else {
                            format!("КАНДИДАТ НА КОД КРАСНЫЙ НЕ УКАЗАН! ОТМЕНА ОПЕРАЦИИ! Пшшшш...")
                        }
                    }
                    1 => {
                        if let Some((user_id, deport_status)) = result.iter().next() {
                            let user_name = user_name(&ctx, user_id)
                                .await
                                .unwrap_or("\"НЕИЗВЕСТНЫЙ ТИП\"".to_string());
                            match deport_status {
                            RedAlertDeportationUserResult::Deported => format!("КОД КРАСНЫЙ ПОДТВЕРЖДЕН! АНТИКРИНЖ ОРУЖИЕ ИСПОЛЬЗОВАНО ПРОТИВ {user_name}!!! 0)00))00"),
                            RedAlertDeportationUserResult::NotFound => {
                                if let Some(suicide_deport_status) = red_alert_handler
                                    .suicide_author_if_possible(&ctx, &msg).await {
                                    match suicide_deport_status {
                                        RedAlertDeportationUserResult::Deported => format!("В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЗНАЧИТ У ТЕБЯ БЕДЫ С БОШКОЙ, КОД КРАСНЫЙ НА ТЕБЯ!"),
                                        RedAlertDeportationUserResult::NotFound => format!("ДОФИГА УМНЫЙ ВИЖУ? В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЖАЛЬ ТЕБЯ В КАНАЛЕ НЕТУ, ТАК БЫ ТЕБЯ ШМАЛЬНУЛ КОДОМ КРАСНЫМ! ОТМЕНА! Пшшшш..."),
                                        RedAlertDeportationUserResult::Error(_) => format!("ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ТО ЧТО ТЫ ПЫТАЛСЯ КИКНУТЬ ТОГО КОГО НЕТ, НО Я СЛОМАЛСЯ! Пшшшш...")
                                    }
                                } else {
                                    format!("КАНДИДАТ НА КОД КРАСНЫЙ НЕ НАЙДЕН! ОТМЕНА ОПЕРАЦИИ! Пшшшш...")
                                }
                            }
                            RedAlertDeportationUserResult::Error(_) => {
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
                        for (user_id, deport_status) in result {
                            let user_name = user_name(&ctx, &user_id)
                                .await
                                .unwrap_or("\"НЕИЗВЕСТНЫЙ ТИП\"".to_string());
                            let deport_status = match deport_status {
                                RedAlertDeportationUserResult::Deported => {
                                    some_kicked = true;
                                    "ИСПОЛНЕНО"
                                }
                                RedAlertDeportationUserResult::NotFound => "НЕ В КАНАЛЕ",
                                RedAlertDeportationUserResult::Error(_) => "ОШИБКА (ПРОЧНЫЙ СУ*А)",
                            };
                            result_strings.push(format!("{user_name} СТАТУС: {deport_status}"))
                        }
                        let result_string = result_strings.join("\n");
                        if some_kicked {
                            format!("ОУ, МАССОВЫЙ КОД КРАСНЫЙ? СТАТУС ВЫКОСА КРИНЖОВИКОВ:\n{result_string}")
                        } else {
                            if let Some(suicide_deport_status) = red_alert_handler
                                .suicide_author_if_possible(&ctx, &msg)
                                .await
                            {
                                match suicide_deport_status {
                                RedAlertDeportationUserResult::Deported => format!("МАССОВЫЙ КОД КРАСНЫЙ ШТУКА ОПАСНАЯ, ТАК КАК ПО РАЗНЫМ ПРИЧИНАМ Я НИКОГО НЕ КИКНУЛ, КИКНУ ТЕБЯ )В)В)))0"),
                                RedAlertDeportationUserResult::NotFound => format!("ЖАЛЬ ТЕБЯ НЕ МОГУ ПРШИТЬ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ! ОТМЕНА Пшшшш..."),
                                RedAlertDeportationUserResult::Error(_) => format!("ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ, НО САМ ОБО****СЯ! Пшшшш...")
                            }
                            } else {
                                format!("ПОЛНЫЙ ПРОВАЛ КОДА КРАСНОГО! ОТМЕНА ОПЕРАЦИИ Пшшшш...")
                            }
                        }
                    }
                }
            }
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
    let args: Vec<String> = std::env::args().collect();
    let token = args.get(1).expect("Missed Discord token in args");
    let mut client = Client::builder(&token)
        .event_handler(Handler::default())
        .await
        .expect("Err creating client");
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
