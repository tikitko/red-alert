use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use serenity::model::prelude::{Message, Ready};
use serenity::model::user::User;
use serenity::prelude::{Context, EventHandler};

#[async_trait]
pub trait OnReady {
    async fn process(&self, ctx: Context, ready: Ready);
}

pub struct CommandParams<'a> {
    pub guild_id: Option<GuildId>,
    pub channel_id: ChannelId,
    pub author: User,
    pub args: &'a [String],
}

pub struct HelpInfo<'a> {
    pub header_suffix: Option<&'a str>,
    pub description: &'a str,
}

#[async_trait]
pub trait Command {
    fn prefix_anchor(&self) -> &str;
    fn help_info<'a>(&'a self) -> Option<HelpInfo<'a>>;
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>);
}

pub struct PrintTextCommand {
    pub prefix_anchor: String,
    pub help_info: Option<(Option<String>, String)>,
    pub text: String,
}

impl PrintTextCommand {
    fn build_help(
        prefix_anchor: String,
        commands: &Vec<Box<dyn Command + Send + Sync + 'static>>,
    ) -> Self {
        Self {
            prefix_anchor,
            help_info: None,
            text: format!(
                "{}",
                commands
                    .iter()
                    .filter_map(|command| {
                        let Some(help_info) = command.help_info() else {
                            return None
                        };
                        Some(format!(
                            "> **`{}`**\n```{}```\n",
                            if let Some(header_suffix) = help_info.header_suffix {
                                format!("{} {}", command.prefix_anchor(), header_suffix)
                            } else {
                                command.prefix_anchor().to_string()
                            },
                            help_info.description
                        ))
                    })
                    .collect::<String>()
            ),
        }
    }
}

#[async_trait]
impl Command for PrintTextCommand {
    fn prefix_anchor(&self) -> &str {
        &self.prefix_anchor
    }
    fn help_info<'a>(&'a self) -> Option<HelpInfo<'a>> {
        let Some(help_info) = &self.help_info else {
            return None;
        };
        Some(HelpInfo {
            header_suffix: {
                let Some(header_suffix) = &help_info.0 else {
                    return None;
                };
                Some(header_suffix.as_str())
            },
            description: help_info.1.as_str(),
        })
    }
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>) {
        let _ = params.channel_id.say(&ctx, &self.text).await;
    }
}

pub struct Handler {
    pub help_command_prefix_anchor: String,
    pub on_ready: Box<dyn OnReady + Send + Sync + 'static>,
    pub commands: Vec<Box<dyn Command + Send + Sync + 'static>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        };
        fn args(string: &str) -> Vec<String> {
            string
                .split(char::is_whitespace)
                .map(|v| v.to_lowercase())
                .collect()
        }
        let help_command: Box<dyn Command + Send + Sync> = Box::new(PrintTextCommand::build_help(
            self.help_command_prefix_anchor.clone(),
            &self.commands,
        ));
        let args_commands = {
            let mut commands = self
                .commands
                .iter()
                .collect::<Vec<&Box<dyn Command + Send + Sync + 'static>>>();
            commands.push(&help_command);
            let mut args_commands: Vec<(Vec<String>, &Box<dyn Command + Send + Sync + 'static>)> =
                vec![];
            for command in commands {
                args_commands.push((args(command.prefix_anchor()), command))
            }
            args_commands.sort_by(|f, s| s.0.len().partial_cmp(&f.0.len()).unwrap());
            args_commands
        };
        let content_args = args(&msg.content);
        for (command_args, command) in args_commands {
            let Some(args) = content_args.strip_prefix(&command_args[..]) else {
                continue;
            };
            let params = CommandParams {
                guild_id: msg.guild_id,
                channel_id: msg.channel_id,
                author: msg.author,
                args,
            };
            command.process(ctx, params).await;
            break;
        }
    }
    async fn ready(&self, ctx: Context, ready: Ready) {
        self.on_ready.process(ctx, ready).await;
    }
}
