use serenity::{
    builder::CreateInteractionResponseMessage,
    model::{
        application::{ResolvedOption, ResolvedValue},
        channel::PartialChannel,
    },
};

pub(crate) fn find_channel<'a>(
    options: &[ResolvedOption<'a>],
    name: &str,
) -> Option<&'a PartialChannel> {
    options.iter().find_map(|option| match option.value {
        ResolvedValue::Channel(channel) if option.name == name => Some(channel),
        _ => None,
    })
}

pub(crate) fn ephemeral(content: impl Into<String>) -> CreateInteractionResponseMessage {
    CreateInteractionResponseMessage::new()
        .content(content)
        .ephemeral(true)
}
