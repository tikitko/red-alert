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

#[async_trait]
pub trait Command {
    fn prefix_anchor(&self) -> &str;
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>);
}

pub struct Handler {
    on_ready: Box<dyn OnReady + Send + Sync + 'static>,
    commands: Vec<Box<dyn Command + Send + Sync + 'static>>,
}

impl Handler {
    pub fn new<R: OnReady + Send + Sync + 'static>(on_ready: R) -> Self {
        Handler {
            on_ready: Box::new(on_ready),
            commands: vec![],
        }
    }
    pub fn push_command<C: Command + Send + Sync + 'static>(&mut self, command: C) {
        self.commands.push(Box::new(command))
    }
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
        let commands = {
            let mut commands = self
                .commands
                .iter()
                .map(|i| (args(i.prefix_anchor()), i))
                .collect::<Vec<(Vec<String>, &Box<dyn Command + Send + Sync + 'static>)>>();
            commands.sort_by(|f, s| s.0.len().partial_cmp(&f.0.len()).unwrap());
            commands
        };
        let content_args = args(&msg.content);
        for (command_args, command) in commands {
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
