mod red_alert_handler;

use std::env;
use guard::*;
use red_alert_handler::*;
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::*;

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

struct Handler {
    red_alert_handler: RedAlertHandler,
}

impl Default for Handler {
    fn default() -> Self {
        Self {
            red_alert_handler: RedAlertHandler,
        }
    }
}

impl Handler {
    async fn listen_for_red_alert(&self, ctx: &Context, channel_id: ChannelId) -> String {
        let channel_name = channel_id.mention();

        format!("ОТСЛЕЖИВАЮ КОД КРАСНЫЙ В КАНАЛЕ: {channel_name}...")
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
        if !is_sub(&message_words, &vec!["код", "красный"]) {
            return;
        }
        let answer_msg = if !msg.mention_channels.is_empty() {
            // self.listen_for_red_alert(&ctx).await
            return
        } else {
            let author_id = msg.author.id;
            let target_users_ids: Vec<UserId> = msg.mentions.iter().map(|u| u.id).collect();
            self.process_red_alert(&ctx, &guild, author_id, target_users_ids).await
        };
        let _ = msg.channel_id.say(&ctx, answer_msg).await;
    }
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");
    let mut client = Client::builder(&token)
        .event_handler(Handler::default())
        .await
        .expect("Err creating client");
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
