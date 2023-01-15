use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use serenity::model::prelude::{Message, Ready};
use serenity::model::user::User;
use serenity::prelude::{Context, EventHandler};

#[async_trait]
pub trait OnReady {
    async fn process(&self, ctx: Context, ready: Ready);
}

#[derive(Clone)]
pub struct HelpInfo {
    pub header_suffix: Option<String>,
    pub description: String,
}

pub struct CommandParams<'a> {
    pub guild_id: Option<GuildId>,
    pub channel_id: ChannelId,
    pub author: User,
    pub args: &'a [String],
}

#[async_trait]
pub trait Command {
    fn prefix_anchor(&self) -> String;
    fn help_info(&self) -> Option<HelpInfo>;
    async fn process<'a>(&'a self, ctx: Context, params: CommandParams<'a>);
}

pub trait HelpCommandFactory {
    fn help_command(
        &self,
        commands_info: Vec<(String, HelpInfo)>,
    ) -> Box<dyn Command + Send + Sync + 'static>;
}

pub struct Handler {
    pub on_ready: Box<dyn OnReady + Send + Sync + 'static>,
    pub commands: Vec<Box<dyn Command + Send + Sync + 'static>>,
    pub help_command_factory: Box<dyn HelpCommandFactory + Send + Sync + 'static>,
}

#[async_trait]
impl EventHandler for Handler {
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
        let help_command = self.help_command_factory.help_command(
            self.commands
                .iter()
                .filter_map(|command| {
                    command
                        .help_info()
                        .map(|help_info| (command.prefix_anchor(), help_info))
                })
                .collect(),
        );
        let args_commands = {
            let mut commands = self
                .commands
                .iter()
                .collect::<Vec<&Box<dyn Command + Send + Sync + 'static>>>();
            commands.push(&help_command);
            let mut args_commands =
                Vec::<(Vec<String>, &Box<dyn Command + Send + Sync + 'static>)>::new();
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
