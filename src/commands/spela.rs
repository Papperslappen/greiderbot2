use std::collections::HashSet;

use serenity::{
    async_trait,
    builder::{
        CreateActionRow, CreateCommand, CreateInteractionResponse,
        CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuKind,
        CreateSelectMenuOption,
    },
    model::{
        application::{CommandInteraction, ComponentInteraction, ComponentInteractionDataKind},
        id::RoleId,
    },
    prelude::Context,
};
use tracing::error;

use super::Command;

const ROLE_PREFIX: &str = "spela ";
// Discord allows at most 25 options in a select menu
const MAX_ROLES: usize = 25;

#[derive(Default)]
pub(crate) struct SpelaCommand;

#[async_trait]
impl Command for SpelaCommand {
    fn name(&self) -> &'static str {
        "spela"
    }

    fn register(&self) -> CreateCommand {
        CreateCommand::new(self.name())
            .name_localized("sv-SE", "spela")
            .description("Join a \"spela\" role ")
            .description_localized("sv-SE", "Gå med i en spela-roll")
    }

    async fn run(&self, command_interaction: CommandInteraction, ctx: &Context) {
        let (Some(guild_id), Some(member)) =
            (command_interaction.guild_id, &command_interaction.member)
        else {
            error!("Command not posted from a guild");
            return;
        };
        let Ok(guild_roles) = guild_id.roles(&ctx.http).await else {
            error!("Could not load roles");
            return;
        };
        let mut suitable_guild_roles = guild_roles
            .into_values()
            .filter(|role| role.name.starts_with(ROLE_PREFIX))
            .collect::<Vec<_>>();
        suitable_guild_roles.sort_by(|a, b| a.name.cmp(&b.name));
        suitable_guild_roles.truncate(MAX_ROLES);

        let message = if suitable_guild_roles.is_empty() {
            CreateInteractionResponseMessage::new()
                .content("Det finns inga spela-roller")
                .ephemeral(true)
        } else {
            let max_values = suitable_guild_roles.len() as u8;
            let options = suitable_guild_roles
                .into_iter()
                .map(|role| {
                    CreateSelectMenuOption::new(role.name, role.id.to_string())
                        .default_selection(member.roles.contains(&role.id))
                })
                .collect();
            let menu = CreateSelectMenu::new(self.name(), CreateSelectMenuKind::String { options })
                .placeholder("Inga roller valda")
                .min_values(0)
                .max_values(max_values);
            CreateInteractionResponseMessage::new()
                .content("Gå med i roll")
                .ephemeral(true)
                .components(vec![CreateActionRow::SelectMenu(menu)])
        };

        if let Err(why) = command_interaction
            .create_response(&ctx.http, CreateInteractionResponse::Message(message))
            .await
        {
            error!("Could not reply to command: {:?}", why);
        }
    }

    async fn component_interaction(&self, interaction: ComponentInteraction, ctx: &Context) {
        let (Some(guild_id), Some(member)) = (interaction.guild_id, &interaction.member) else {
            error!("Component interaction not performed by a guild member");
            return;
        };
        let ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind else {
            error!("Unexpected component kind: {:?}", &interaction.data.kind);
            return;
        };
        let Ok(guild_roles) = guild_id.roles(&ctx.http).await else {
            error!("Could not fetch roles");
            return;
        };
        let is_play_role = |id: &RoleId| {
            guild_roles
                .get(id)
                .is_some_and(|role| role.name.starts_with(ROLE_PREFIX))
        };

        // Never trust the submitted values to be roles that were actually offered
        let selected_roleids = values
            .iter()
            .filter_map(|value| value.parse::<RoleId>().ok())
            .filter(is_play_role)
            .collect::<HashSet<_>>();

        let user_current_play_roleids = member
            .roles
            .iter()
            .copied()
            .filter(is_play_role)
            .collect::<HashSet<_>>();

        let remove_roleids = user_current_play_roleids
            .difference(&selected_roleids)
            .copied()
            .collect::<Vec<_>>();
        let add_roleids = selected_roleids
            .difference(&user_current_play_roleids)
            .copied()
            .collect::<Vec<_>>();

        let (Ok(_), Ok(_)) = (
            member.add_roles(&ctx.http, &add_roleids).await,
            member.remove_roles(&ctx.http, &remove_roleids).await,
        ) else {
            error!("Could not update roles of user");
            return;
        };

        let message = CreateInteractionResponseMessage::new()
            .ephemeral(true)
            .content(format!(
                "La till rollerna: {}. \n Tog bort rollerna: {}",
                add_roleids
                    .iter()
                    .map(mention_role)
                    .collect::<Vec<_>>()
                    .join(", "),
                remove_roleids
                    .iter()
                    .map(mention_role)
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        if let Err(why) = interaction
            .create_response(&ctx.http, CreateInteractionResponse::Message(message))
            .await
        {
            error!("Could not send response: {:?}", why);
        }
    }
}

fn mention_role(role: &RoleId) -> String {
    format!("<@&{}>", role)
}
