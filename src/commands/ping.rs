use crate::context_ext::ContextExt;

use super::interaction_handler::{CommandHandler, InteractionHandler};
use serenity::{
    async_trait,
    client::Context,
    model::interactions::{
        application_command::ApplicationCommandInteraction, InteractionResponseType,
    },
};
use tracing::log::warn;

#[derive(Clone, Copy)]
pub struct Ping;

impl InteractionHandler for Ping {
    fn name(&self) -> &'static str {
        "ping"
    }
}

#[async_trait]
impl CommandHandler for Ping {
    async fn invoke(&self, ctx: Context, interaction: ApplicationCommandInteraction) {
        let content = format!("{:#?}", ctx.session().await.read().await.users);
        if let Err(why) = interaction
            .create_interaction_response(&ctx.http, |response| {
                response
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| message.content(content))
            })
            .await
        {
            warn!("Error responding to slash command: {}", why);
        }
    }

    fn create_command(
        self,
        command: &mut serenity::builder::CreateApplicationCommand,
    ) -> &mut serenity::builder::CreateApplicationCommand {
        command.name(self.name()).description("A ping/pong command")
    }
}
