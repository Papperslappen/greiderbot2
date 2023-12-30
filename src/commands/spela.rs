use std::collections::HashSet;

use serenity::{
    async_trait,
    model::prelude::{
        interaction::{
            application_command::ApplicationCommandInteraction,
            message_component::MessageComponentInteraction,
        },
        RoleId,
    },
    prelude::Context,
};
use tracing::error;

use super::Command;

#[derive(Default)]
pub(crate) struct SpelaCommand;

#[async_trait]
impl Command for SpelaCommand {
    fn name(&self) -> String {
        "spela".to_string()
    }

    fn register<'a>(
        &self,
        command: &'a mut serenity::builder::CreateApplicationCommand,
    ) -> &'a mut serenity::builder::CreateApplicationCommand {
        command
            .name("spela")
            .name_localized("sv-SE", "spela")
            .description("Join a \"spela\" role ")
            .description_localized("sv-SE", "Gå med i en spela-roll")
    }

    async fn run(&self, command_interaction: ApplicationCommandInteraction, ctx: &Context) {
        let prefix = "spela ";

        let Some(guild_id) = command_interaction.guild_id else {
            panic!("Command not posted from a guild")
        };
        let Ok(suitable_guild_roles) = guild_id.to_partial_guild(&ctx).await.map(|guild| {
            guild
                .roles
                .into_values()
                .filter(|role| role.name.starts_with(prefix))
                .take(25)
                .collect::<Vec<_>>()
        }) else {
            error!("Could not load roles");
            return;
        };

        command_interaction
            .create_interaction_response(&ctx.http, |response| {
                response
                    .kind(serenity::model::prelude::interaction::InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|data| {
                        data.content("Gå med i roll")
                            .custom_id(self.name())
                            .ephemeral(true)
                            .title("Spela")
                            .components(|component| {
                                component.create_action_row(|row| {
                                    row.create_select_menu(|menu| {
                                        menu.custom_id(self.name())
                                            .placeholder("Inga roller valda")
                                            .max_values(suitable_guild_roles.len() as u64)
                                            .min_values(0);
                                        menu.options(|options| {
                                            for role in suitable_guild_roles {
                                                options.create_option(|option| {
                                                    option
                                                        .label(role.name)
                                                        .value(role.id)
                                                        .default_selection(
                                                            command_interaction.member.clone().expect("Interaction not performed by guild member").roles.contains(&role.id)
                                                        )
                                                });
                                            }
                                            options
                                        })
                                    })
                                });
                                component
                            })
                    })
                //.interaction_response_data(|data| data.content(suitable_guild_roles.join(", ")))
            })
            .await
            .expect("Could not reply to message");

        // let Some(Ok(guild)) = guild
        // else {
        //
        // };
        // let play_roles = guild.roles.values().map(|role| role.name.clone()).collect::<Vec<_>>();
        // if let (guild_roles) = command
        //     .guild_id.unwrap()
        //     .to_partial_guild(ctx)
        //     .await.unwrap()
        //     .roles.values().map(|role| role.name.clone()).collect::<Vec<_>>(){

        //     }
    }

    async fn component_interaction(&self, interaction: MessageComponentInteraction, ctx: &Context) {
        let Ok(guild_roles) = interaction
            .guild_id
            .expect("Command interaction lacks a guild")
            .roles(&ctx)
            .await
        else {
            error!("Could not fetch roles");
            return;
        };

        let mut member = interaction
            .member
            .clone()
            .expect("Could not get guild member from interaction");

        let selected_roleids = interaction
            .data
            .values
            .iter()
            .map(|value| RoleId::from(value.parse::<u64>().expect("Could not parse value")))
            //.map(|id| guild_roles.get(&id))
            .collect::<HashSet<_>>();

        let member_curent_roleids = member.roles.clone();

        let user_current_play_rolids = member_curent_roleids
            .into_iter()
            .filter(|id| {
                guild_roles
                    .get(id)
                    .expect("Strange! guild member of non guild role")
                    .name
                    .starts_with("spela ")
            })
            .collect::<HashSet<_>>();

        let remove_roleids = user_current_play_rolids
            .difference(&selected_roleids)
            .cloned()
            .collect::<Vec<_>>();
        let add_roleids = selected_roleids
            .difference(&user_current_play_rolids)
            .cloned()
            .collect::<Vec<_>>();

        let (Ok(_), Ok(_)) = (
            member.add_roles(ctx, &add_roleids).await,
            member.remove_roles(ctx, &remove_roleids).await,
        ) else {
            error!("Could not update roles of user");
            return;
        };

        interaction
            .create_interaction_response(&ctx, |response| {
                response.interaction_response_data(|data| {
                    data.ephemeral(true).content(format!(
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
                    ))
                })
            })
            .await
            .expect("Could not send response");
    }
}

fn mention_role(role: &RoleId) -> String {
    format!("<@&{}>", role)
}
