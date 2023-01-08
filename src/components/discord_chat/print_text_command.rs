use super::*;
use serenity::prelude::Context;

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
