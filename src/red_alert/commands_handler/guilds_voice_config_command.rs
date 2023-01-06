use super::*;
use serenity::model::prelude::Mention;
use serenity::model::prelude::UserId;
use serenity::prelude::{Context, Mentionable};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(super) struct GuildsVoiceConfigRedAlertCommand {
    pub(super) guilds_voice_config: Arc<RwLock<RedAlertGuildsVoiceConfig>>,
}

impl GuildsVoiceConfigRedAlertCommand {
    fn process_self_words(
        guild_voice_config: &mut RedAlertVoiceConfig<u64>,
        args: Vec<String>,
    ) -> String {
        let word = args.join(SPACE);
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
        let word = args.join(SPACE);
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
        let word = args.join(SPACE);
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
            "Запретные:\n{}\nВыгоняющие:\n{}\nПсевдонимы:\n{}\nПогрешность: {}",
            guild_voice_config
                .self_words
                .iter()
                .map(|a| format!(" - {}", a))
                .collect::<Vec<String>>()
                .join(NEW_LINE),
            guild_voice_config
                .target_words
                .iter()
                .map(|a| format!(" - {}", a))
                .collect::<Vec<String>>()
                .join(NEW_LINE),
            guild_voice_config
                .aliases
                .iter()
                .map(|a| format!(" - {}: {}", a.0, UserId(*a.1).mention()))
                .collect::<Vec<String>>()
                .join(NEW_LINE),
            guild_voice_config.similarity_threshold,
        )
    }
}

#[async_trait]
impl Command for GuildsVoiceConfigRedAlertCommand {
    fn prefix_anchor(&self) -> String {
        "код красный фраза".to_string()
    }
    fn help_info(&self) -> Option<HelpInfo> {
        Some(HelpInfo {
            header_suffix: Some("[запретная/выгоняющая/псевдоним/погрешность/список]".to_string()),
            description:
                "[запретная] {фраза} - добавляет/удаляет фразу при призношении которой пользователь будет исключен.\n[выгоняющая] {фраза} - добавляет/удаляет фразу при призношении которой пользователь может исключить другого пользователя.\n[псевдоним] {фраза} {ID или упоминание пользователя} - добавляет/удаляет псевдоним для пользователя который можно использовать в распознавателе речи.\n[погрешность] {0.0 - 1.0} - устанавливает погрешность разпознавания речи.\n[список] - список всех фраз.".to_string(),
        })
    }
    async fn process<'a>(&'a self, ctx: Context, params: CommandParams<'a>) {
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
