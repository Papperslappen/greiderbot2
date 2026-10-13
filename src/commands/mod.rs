use serenity::{
    async_trait,
    builder::CreateCommand,
    model::application::{CommandInteraction, ComponentInteraction},
    prelude::Context,
};

pub(crate) mod blanda;
pub(crate) mod spela;

#[async_trait]
pub(crate) trait Command: Sync + Send {
    fn name(&self) -> &'static str;
    fn register(&self) -> CreateCommand;
    async fn run(&self, command: CommandInteraction, ctx: &Context);
    async fn component_interaction(&self, _interaction: ComponentInteraction, _ctx: &Context) {}
}
