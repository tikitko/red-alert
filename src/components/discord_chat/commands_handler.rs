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

#[derive(Clone)]
pub struct HelpInfo {
    pub header_suffix: Option<String>,
    pub description: String,
}

#[async_trait]
pub trait Command {
    fn prefix_anchor(&self) -> String;
    fn help_info(&self) -> Option<HelpInfo>;
    async fn process<'a>(&'a self, ctx: Context, params: CommandParams<'a>);
}

pub struct PrintTextCommand {
    pub prefix_anchor: String,
    pub help_info: Option<HelpInfo>,
    pub text: String,
}

#[async_trait]
impl Command for PrintTextCommand {
    fn prefix_anchor(&self) -> String {
        self.prefix_anchor.clone()
    }
    fn help_info(&self) -> Option<HelpInfo> {
        self.help_info.clone()
    }
    async fn process<'a>(&'a self, ctx: Context, params: CommandParams<'a>) {
        let _ = params.channel_id.say(&ctx, &self.text).await;
    }
}

pub struct HelpCommandConfig<F: Fn(String, HelpInfo) -> String + Send + Sync + 'static> {
    pub prefix_anchor: String,
    pub output_prefix: Option<String>,
    pub output_format_fn: F,
}

pub struct Handler<F: Fn(String, HelpInfo) -> String + Send + Sync + 'static> {
    pub help_command: HelpCommandConfig<F>,
    pub on_ready: Box<dyn OnReady + Send + Sync + 'static>,
    pub commands: Vec<Box<dyn Command + Send + Sync + 'static>>,
}

#[async_trait]
impl<F: Fn(String, HelpInfo) -> String + Send + Sync + 'static> EventHandler for Handler<F> {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        };
        fn args(string: String) -> Vec<String> {
            string
                .split(char::is_whitespace)
                .map(str::to_lowercase)
                .collect()
        }
        let help_command: Box<dyn Command + Send + Sync> = Box::new(PrintTextCommand {
            prefix_anchor: self.help_command.prefix_anchor.clone(),
            help_info: None,
            text: {
                let mut help_text = self
                    .commands
                    .iter()
                    .filter_map(|command| {
                        command.help_info().map(|help_info| {
                            (self.help_command.output_format_fn)(command.prefix_anchor(), help_info)
                        })
                    })
                    .collect::<String>();
                if let Some(output_prefix) = &self.help_command.output_prefix {
                    help_text.insert_str(0, output_prefix);
                }
                help_text
            },
        });
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
        let content_args = args(msg.content);
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
