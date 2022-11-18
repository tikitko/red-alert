use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use serenity::model::prelude::{Message, Ready};
use serenity::model::user::User;
use serenity::prelude::{Context, EventHandler};
use std::cmp::Ordering;
use std::collections::HashMap;

#[async_trait]
pub trait OnReady {
    async fn process(&self, ctx: Context, ready: Ready);
}

pub struct CommandParams<'a> {
    pub guild_id: Option<GuildId>,
    pub channel_id: ChannelId,
    pub author: User,
    pub args: &'a [&'a str],
}

#[async_trait]
pub trait Command {
    async fn process<'a>(&self, ctx: Context, params: CommandParams<'a>);
}

pub struct Handler {
    on_ready: Box<dyn OnReady + Send + Sync + 'static>,
    commands: HashMap<String, Box<dyn Command + Send + Sync + 'static>>,
}

impl Handler {
    pub fn new<R: OnReady + Send + Sync + 'static>(on_ready: R) -> Self {
        Handler {
            on_ready: Box::new(on_ready),
            commands: HashMap::default(),
        }
    }
    pub fn insert_command<C: Command + Send + Sync + 'static>(&mut self, key: String, command: C) {
        self.commands.insert(key, Box::new(command));
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        };
        fn args(string: &String) -> Vec<&str> {
            string.split(char::is_whitespace).collect::<Vec<&str>>()
        }
        let mut args_commands = self
            .commands
            .iter()
            .map(|i| (args(i.0), i.1))
            .collect::<Vec<(Vec<&str>, &Box<dyn Command + Send + Sync + 'static>)>>();
        args_commands.sort_by(|f, s| {
            let f_len = f.0.len();
            let s_len = s.0.len();
            if f_len == s_len {
                Ordering::Equal
            } else if f_len > s_len {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        });
        let content_args = args(&msg.content);
        for (args, command) in args_commands {
            let Some(args) = content_args.as_slice().strip_prefix(args.as_slice()) else {
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
