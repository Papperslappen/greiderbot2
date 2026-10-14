use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serenity::{
    async_trait,
    builder::{
        CreateCommand, CreateCommandOption, CreateEmbed, CreateInteractionResponse,
        CreateInteractionResponseMessage, CreateMessage, EditChannel,
    },
    model::{
        Colour, Timestamp,
        application::{CommandInteraction, CommandOptionType, ResolvedOption, ResolvedValue},
        channel::{ChannelType, PartialChannel},
        id::{ChannelId, GuildId},
        permissions::Permissions,
    },
    prelude::{Context, Mutex},
};
use tracing::{error, info};

use super::{
    Command,
    util::{ephemeral, find_channel},
};

const CHANNEL_SUBCOMMAND: &str = "channel";
const CATEGORY_SUBCOMMAND: &str = "category";
const CHANNEL_OPTION: &str = "channel";
const CATEGORY_OPTION: &str = "category";
const REMOVAL_DELAY: Duration = Duration::from_secs(14 * 24 * 60 * 60);
const INACTIVITY_LIMIT: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// Moves channels to a "soon to be deleted" category, chosen per guild with
/// `/retire category`. The chosen categories are stored in a JSON file.
pub(crate) struct RetireCommand {
    path: PathBuf,
    categories: Mutex<HashMap<GuildId, ChannelId>>,
}

impl RetireCommand {
    pub(crate) fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let categories = match std::fs::read_to_string(&path) {
            Ok(json) => serde_json::from_str::<HashMap<u64, u64>>(&json)
                .unwrap_or_else(|why| panic!("Could not parse {}: {}", path.display(), why))
                .into_iter()
                .map(|(guild, category)| (GuildId::new(guild), ChannelId::new(category)))
                .collect(),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(why) => panic!("Could not read {}: {}", path.display(), why),
        };
        RetireCommand {
            path,
            categories: Mutex::new(categories),
        }
    }

    async fn set_category(
        &self,
        guild_id: GuildId,
        category: ChannelId,
    ) -> Result<(), std::io::Error> {
        let mut categories = self.categories.lock().await;
        categories.insert(guild_id, category);
        let json = serde_json::to_string_pretty(
            &categories
                .iter()
                .map(|(guild, category)| (guild.get(), category.get()))
                .collect::<HashMap<_, _>>(),
        )?;
        // Write to a temporary file first so a crash can't leave a half written file
        let tmp_path = self.path.with_extension("tmp");
        tokio::fs::write(&tmp_path, json).await?;
        tokio::fs::rename(&tmp_path, &self.path).await
    }

    async fn retire_channel(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        guild_id: GuildId,
        channel: &PartialChannel,
    ) -> CreateInteractionResponseMessage {
        let Some(category) = self.categories.lock().await.get(&guild_id).copied() else {
            return ephemeral(
                "Ingen pensionskategori är vald. Välj en med `/pensionera kategori` först.",
            );
        };
        if channel.parent_id == Some(category) {
            return ephemeral(format!("<#{}> är redan pensionerad.", channel.id));
        }

        let last_message_id = match channel.id.to_channel(&ctx.http).await {
            Ok(fetched) => fetched.guild().and_then(|fetched| fetched.last_message_id),
            Err(why) => {
                error!("Could not fetch channel {}: {:?}", channel.id, why);
                return ephemeral(format!("Kunde inte hämta <#{}>.", channel.id));
            }
        };
        // Message ids encode when the message was posted. For forums this is the latest
        // post, and channels without messages count from when they were created.
        let last_activity = last_message_id
            .map_or(channel.id.created_at(), |id| id.created_at())
            .unix_timestamp();
        let inactive_since = unix_now() - INACTIVITY_LIMIT.as_secs() as i64;
        if last_activity > inactive_since {
            return ephemeral(format!(
                "<#{}> kan inte pensioneras, det senaste meddelandet skrevs <t:{last_activity}:R>. \
                 Kanaler kan bara pensioneras när ingen har skrivit i dem på 14 dagar.",
                channel.id
            ));
        }

        if let Err(why) = channel
            .id
            .edit(&ctx.http, EditChannel::new().category(category))
            .await
        {
            error!("Could not move channel {}: {:?}", channel.id, why);
            return ephemeral(format!(
                "Kunde inte flytta <#{}> till <#{}>. Har boten behörighet att hantera kanaler, och finns kategorin kvar?",
                channel.id, category
            ));
        }
        info!("Retired channel {} in guild {}", channel.id, guild_id);

        let removal = unix_now() + REMOVAL_DELAY.as_secs() as i64;
        let notice = CreateEmbed::new()
            .title("🌅 Kanalen har gått i pension")
            .description(
                "Ingen har skrivit här på ett tag, så kanalen har flyttats till pensionskategorin. \
                 Den tas bort om två veckor.",
            )
            .colour(Colour::GOLD)
            .field(
                "Tas bort",
                format!("<t:{removal}:D> (<t:{removal}:R>)"),
                true,
            )
            .field("Senaste aktivitet", format!("<t:{last_activity}:R>"), true)
            .field("Pensionerad av", format!("<@{}>", command.user.id), true)
            .timestamp(Timestamp::now());

        if command.channel_id == channel.id {
            return CreateInteractionResponseMessage::new().embed(notice);
        }
        if let Err(why) = channel
            .id
            .send_message(&ctx.http, CreateMessage::new().embed(notice))
            .await
        {
            error!("Could not post notice in {}: {:?}", channel.id, why);
            return ephemeral(format!(
                "Flyttade <#{}> men kunde inte skriva meddelandet om pensionen i kanalen.",
                channel.id
            ));
        }
        ephemeral(format!("Pensionerade <#{}>.", channel.id))
    }
}

#[async_trait]
impl Command for RetireCommand {
    fn name(&self) -> &'static str {
        "retire"
    }

    fn register(&self) -> CreateCommand {
        CreateCommand::new(self.name())
            .name_localized("sv-SE", "pensionera")
            .description("Retire a channel that will soon be removed")
            .description_localized("sv-SE", "Pensionera en kanal som snart ska tas bort")
            .default_member_permissions(Permissions::MANAGE_CHANNELS)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    CHANNEL_SUBCOMMAND,
                    "Move a channel to the retirement category",
                )
                .name_localized("sv-SE", "kanal")
                .description_localized("sv-SE", "Flytta en kanal till pensionskategorin")
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::Channel,
                        CHANNEL_OPTION,
                        "The channel to retire, the current channel if left out",
                    )
                    .name_localized("sv-SE", "kanal")
                    .description_localized(
                        "sv-SE",
                        "Kanalen som ska pensioneras, den här kanalen om ingen anges",
                    )
                    .channel_types(vec![
                        ChannelType::Text,
                        ChannelType::Voice,
                        ChannelType::News,
                        ChannelType::Stage,
                        ChannelType::Forum,
                    ]),
                ),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    CATEGORY_SUBCOMMAND,
                    "Choose the category retired channels are moved to",
                )
                .name_localized("sv-SE", "kategori")
                .description_localized(
                    "sv-SE",
                    "Välj kategorin som pensionerade kanaler flyttas till",
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::Channel,
                        CATEGORY_OPTION,
                        "The retirement category",
                    )
                    .name_localized("sv-SE", "kategori")
                    .description_localized("sv-SE", "Pensionskategorin")
                    .channel_types(vec![ChannelType::Category])
                    .required(true),
                ),
            )
    }

    async fn run(&self, command: CommandInteraction, ctx: &Context) {
        let Some(guild_id) = command.guild_id else {
            error!("Command not posted from a guild");
            return;
        };

        // default_member_permissions can be overridden by the server, so check here as well
        let allowed = command
            .member
            .as_ref()
            .and_then(|member| member.permissions)
            .is_some_and(|permissions| permissions.manage_channels());

        let options = command.data.options();
        let message = match options.first() {
            _ if !allowed => ephemeral("Du behöver behörighet att hantera kanaler."),
            Some(ResolvedOption {
                name: CHANNEL_SUBCOMMAND,
                value: ResolvedValue::SubCommand(sub_options),
                ..
            }) => {
                let channel = match find_channel(sub_options, CHANNEL_OPTION) {
                    Some(channel) => Some(channel),
                    None => command.channel.as_ref(),
                };
                match channel {
                    Some(channel) => self.retire_channel(ctx, &command, guild_id, channel).await,
                    None => ephemeral("Kunde inte hitta kanalen."),
                }
            }
            Some(ResolvedOption {
                name: CATEGORY_SUBCOMMAND,
                value: ResolvedValue::SubCommand(sub_options),
                ..
            }) => match find_channel(sub_options, CATEGORY_OPTION) {
                Some(category) => match self.set_category(guild_id, category.id).await {
                    Ok(()) => ephemeral(format!(
                        "Pensionerade kanaler flyttas nu till <#{}>.",
                        category.id
                    )),
                    Err(why) => {
                        error!("Could not save pension category: {:?}", why);
                        ephemeral("Kunde inte spara pensionskategorin.")
                    }
                },
                None => ephemeral("Kunde inte hitta kategorin."),
            },
            _ => {
                error!("Unexpected options: {:?}", options);
                return;
            }
        };

        if let Err(why) = command
            .create_response(&ctx.http, CreateInteractionResponse::Message(message))
            .await
        {
            error!("Could not reply to command: {:?}", why);
        }
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock is before 1970")
        .as_secs() as i64
}
