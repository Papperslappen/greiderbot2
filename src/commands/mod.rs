use serenity::{
    async_trait,
    builder::CreateApplicationCommand,
    model::prelude::interaction::{
        application_command::ApplicationCommandInteraction,
        message_component::MessageComponentInteraction,
    },
    prelude::Context,
};

pub(crate) mod spela;

#[async_trait]
pub(crate) trait Command: Sync + Send {
    fn name(&self) -> String;
    fn register<'a>(
        &self,
        command: &'a mut CreateApplicationCommand,
    ) -> &'a mut CreateApplicationCommand;
    async fn run(&self, command: ApplicationCommandInteraction, ctx: &Context);
    async fn component_interaction(
        &self,
        _interaction: MessageComponentInteraction,
        _ctx: &Context,
    ) {
    }
}
