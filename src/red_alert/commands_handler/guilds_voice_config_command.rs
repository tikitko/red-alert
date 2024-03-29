use super::*;
use serenity::model::prelude::Mention;
use serenity::model::prelude::UserId;
use serenity::prelude::{Context, Mentionable};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(super) struct GuildsVoiceConfigRedAlertCommand {
    pub(super) guilds_voice_config: Arc<RwLock<RedAlertGuildsVoiceConfig>>,
    pub(super) l10n: L10n,
}

fn process_self_words(
    l10n: &L10n,
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
        l10n.string(
            "guilds-voice-config-red-alert-command-self-words-remove",
            fluent_args![],
        )
    } else {
        guild_voice_config.self_words.push(word);
        l10n.string(
            "guilds-voice-config-red-alert-command-self-words-add",
            fluent_args![],
        )
    }
}
fn process_target_words(
    l10n: &L10n,
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
        l10n.string(
            "guilds-voice-config-red-alert-command-target-words-remove",
            fluent_args![],
        )
    } else {
        guild_voice_config.target_words.push(word);
        l10n.string(
            "guilds-voice-config-red-alert-command-target-words-add",
            fluent_args![],
        )
    }
}
fn process_aliases(
    l10n: &L10n,
    guild_voice_config: &mut RedAlertVoiceConfig<u64>,
    mut args: Vec<String>,
) -> String {
    if !(args.len() > 1) {
        return l10n.string(
            "guilds-voice-config-red-alert-command-aliases-empty-params",
            fluent_args![],
        );
    }
    let user_id_string = args.remove(args.len() - 1);
    let Some(user_id) = (match Mention::from_str(&*user_id_string) {
        Ok(mention) => match mention {
            Mention::User(user_id) => Some(user_id),
            Mention::Channel(_) | Mention::Role(_) | Mention::Emoji(_, _) => None,
        },
        Err(_) => user_id_string.parse::<u64>().ok().map(UserId),
    }) else {
        return l10n.string(
            "guilds-voice-config-red-alert-command-aliases-incorrect-user",
            fluent_args![],
        )
    };
    let word = args.join(SPACE);
    let Some(saved_user_id) = guild_voice_config.aliases.remove(&word) else {
        guild_voice_config.aliases.insert(word, user_id.0);
        return l10n.string(
            "guilds-voice-config-red-alert-command-aliases-add",
            fluent_args![
                "user-name" => user_id.mention().to_string()
            ],
        )
    };
    if saved_user_id == user_id.0 {
        l10n.string(
            "guilds-voice-config-red-alert-command-aliases-remove",
            fluent_args![
                "user-name" => user_id.mention().to_string()
            ],
        )
    } else {
        guild_voice_config.aliases.insert(word, user_id.0);
        l10n.string(
            "guilds-voice-config-red-alert-command-aliases-replace",
            fluent_args![
                "user-name" => user_id.mention().to_string()
            ],
        )
    }
}
fn process_similarity_threshold(
    l10n: &L10n,
    guild_voice_config: &mut RedAlertVoiceConfig<u64>,
    mut args: Vec<String>,
) -> String {
    if !(args.len() > 0) {
        return l10n.string(
            "guilds-voice-config-red-alert-command-similarity-threshold-empty-params",
            fluent_args![],
        );
    }
    let similarity_threshold_string = args.remove(0);
    let Ok(similarity_threshold) = similarity_threshold_string.parse::<f32>() else {
        return l10n.string(
            "guilds-voice-config-red-alert-command-similarity-threshold-incorrect-params",
            fluent_args![],
        )
    };
    let similarity_threshold = similarity_threshold.max(0.0).min(1.0);
    guild_voice_config.similarity_threshold = similarity_threshold;
    l10n.string(
        "guilds-voice-config-red-alert-command-similarity-threshold-success",
        fluent_args![
            "similarity-threshold" => similarity_threshold
        ],
    )
}
fn process_editors(
    l10n: &L10n,
    guild_voice_config: &mut RedAlertVoiceConfig<u64>,
    mut args: Vec<String>,
) -> String {
    if !(args.len() > 0) {
        return l10n.string(
            "guilds-voice-config-red-alert-command-editors-empty-params",
            fluent_args![],
        );
    }
    let user_id_string = args.remove(0);
    let Some(user_id) = (match Mention::from_str(&*user_id_string) {
        Ok(mention) => match mention {
            Mention::User(user_id) => Some(user_id),
            Mention::Channel(_) | Mention::Role(_) | Mention::Emoji(_, _) => None,
        },
        Err(_) => user_id_string.parse::<u64>().ok().map(UserId),
    }) else {
        return l10n.string(
            "guilds-voice-config-red-alert-command-editors-incorrect-user",
            fluent_args![],
        )
    };
    let mut editors = guild_voice_config.editors.clone().unwrap_or_default();
    let answer: String;
    if editors.contains(&user_id.0) {
        if editors.len() > 1 {
            editors.remove(&user_id.0);
            answer = l10n.string(
                "guilds-voice-config-red-alert-command-editors-remove",
                fluent_args![],
            );
        } else {
            answer = l10n.string(
                "guilds-voice-config-red-alert-command-editors-one-error",
                fluent_args![],
            );
        }
    } else {
        editors.insert(user_id.0);
        answer = l10n.string(
            "guilds-voice-config-red-alert-command-editors-add",
            fluent_args![],
        );
    }
    guild_voice_config.editors = Some(editors);
    answer
}
fn process_list(l10n: &L10n, guild_voice_config: &RedAlertVoiceConfig<u64>) -> String {
    l10n.string(
        "guilds-voice-config-red-alert-command-list-template",
        fluent_args![
            "self-words" => guild_voice_config
                .self_words
                .iter()
                .map(|record| l10n.string(
                    "guilds-voice-config-red-alert-command-list-record-single",
                    fluent_args![
                        "record" => record.clone()
                    ],
                ))
                .collect::<Vec<String>>()
                .join(NEW_LINE),
            "target-words" => guild_voice_config
                .target_words
                .iter()
                .map(|record| l10n.string(
                    "guilds-voice-config-red-alert-command-list-record-single",
                    fluent_args![
                        "record" => record.clone()
                    ],
                ))
                .collect::<Vec<String>>()
                .join(NEW_LINE),
            "aliases" => guild_voice_config
                .aliases
                .iter()
                .map(|record| l10n.string(
                    "guilds-voice-config-red-alert-command-list-record-double",
                    fluent_args![
                        "record-start" => record.0.clone(),
                        "record-end" => UserId(*record.1).mention().to_string()
                    ],
                ))
                .collect::<Vec<String>>()
                .join(NEW_LINE)
        ],
    )
}

enum Action {
    SelfWords,
    TargetWords,
    Aliases,
    SimilarityThreshold,
    Editors,
    List,
}

impl Action {
    fn process(
        &self,
        l10n: &L10n,
        guild_voice_config: &mut RedAlertVoiceConfig<u64>,
        args: Vec<String>,
    ) -> String {
        match self {
            Action::SelfWords => process_self_words(l10n, guild_voice_config, args),
            Action::TargetWords => process_target_words(l10n, guild_voice_config, args),
            Action::Aliases => process_aliases(l10n, guild_voice_config, args),
            Action::SimilarityThreshold => {
                process_similarity_threshold(l10n, guild_voice_config, args)
            }
            Action::Editors => process_editors(l10n, guild_voice_config, args),
            Action::List => process_list(l10n, guild_voice_config),
        }
    }
}

#[async_trait]
impl Command for GuildsVoiceConfigRedAlertCommand {
    fn prefix_anchor(&self) -> String {
        self.l10n.string(
            "guilds-voice-config-red-alert-command-prefix-anchor",
            fluent_args![],
        )
    }
    fn help_info(&self) -> Option<HelpInfo> {
        Some(HelpInfo {
            header_suffix: Some(self.l10n.string(
                "guilds-voice-config-red-alert-command-header-suffix",
                fluent_args![],
            )),
            description: self.l10n.string(
                "guilds-voice-config-red-alert-command-help-description",
                fluent_args![],
            ),
        })
    }
    async fn process<'a>(&'a self, ctx: Context, params: CommandParams<'a>) {
        let Some(guild_id) = params.guild_id else {
            return;
        };
        let mut guilds_voice_config = self.guilds_voice_config.write().await;
        let mut guild_voice_config = guilds_voice_config.remove(&guild_id);
        let mut args = params.args.to_vec();
        let answer_msg = {
            let access_granted = guild_voice_config
                .editors
                .as_ref()
                .map_or_else(|| true, |e| e.contains(&params.author.id.0));
            if !access_granted {
                self.l10n.string(
                    "guilds-voice-config-red-alert-command-no-access",
                    fluent_args![],
                )
            } else if !(args.len() > 0) {
                self.l10n.string(
                    "guilds-voice-config-red-alert-command-empty-action",
                    fluent_args![],
                )
            } else {
                let actions: HashMap<String, Action> = HashMap::from([
                    (
                        self.l10n.string(
                            "guilds-voice-config-red-alert-command-self-words-action",
                            fluent_args![],
                        ),
                        Action::SelfWords,
                    ),
                    (
                        self.l10n.string(
                            "guilds-voice-config-red-alert-command-target-words-action",
                            fluent_args![],
                        ),
                        Action::TargetWords,
                    ),
                    (
                        self.l10n.string(
                            "guilds-voice-config-red-alert-command-aliases-action",
                            fluent_args![],
                        ),
                        Action::Aliases,
                    ),
                    (
                        self.l10n.string(
                            "guilds-voice-config-red-alert-command-similarity-threshold-action",
                            fluent_args![],
                        ),
                        Action::SimilarityThreshold,
                    ),
                    (
                        self.l10n.string(
                            "guilds-voice-config-red-alert-command-editors-action",
                            fluent_args![],
                        ),
                        Action::Editors,
                    ),
                    (
                        self.l10n.string(
                            "guilds-voice-config-red-alert-command-list-action",
                            fluent_args![],
                        ),
                        Action::List,
                    ),
                ]);
                let action_string = args.remove(0);
                if let Some(action) = actions.get(&action_string) {
                    action.process(&self.l10n, &mut guild_voice_config, args)
                } else if self.l10n.string(
                    "guilds-voice-config-red-alert-command-auto-track-action",
                    fluent_args![],
                ) == action_string
                {
                    if guilds_voice_config.switch_auto_track(guild_id) {
                        self.l10n.string(
                            "guilds-voice-config-red-alert-command-auto-track-add",
                            fluent_args![],
                        )
                    } else {
                        self.l10n.string(
                            "guilds-voice-config-red-alert-command-auto-track-remove",
                            fluent_args![],
                        )
                    }
                } else {
                    self.l10n.string(
                        "guilds-voice-config-red-alert-command-incorrect-action",
                        fluent_args![],
                    )
                }
            }
        };
        guilds_voice_config.insert(guild_id, guild_voice_config);
        guilds_voice_config.write();
        drop(guilds_voice_config);
        let _ = params.channel_id.say(&ctx, answer_msg).await;
    }
}
