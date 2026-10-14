use std::collections::HashMap;
use std::env;

use clap::Parser;

use commands::blanda::BlandaCommand;
use commands::retire::RetireCommand;
use commands::spela::SpelaCommand;
use serenity::model::application::Interaction;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use serenity::{async_trait, futures::future::join_all};
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

mod commands;

#[derive(Debug, Parser)]
#[clap(name = "Greidebot", about = "A discord bot")]
struct Opt {
    #[clap(short = 'l', long = "log-level", default_value = "debug")]
    log_level: String,
}

#[derive(Default)]
struct GreiderbotBuilder {
    commands: Vec<Box<dyn crate::commands::Command>>,
}

impl GreiderbotBuilder {
    fn add_command<T: crate::commands::Command + 'static>(mut self, command: T) -> Self {
        self.commands.push(Box::new(command));
        self
    }
    fn build(self) -> Greiderbot {
        Greiderbot::from_commands(self.commands)
    }
}

struct Greiderbot {
    commands: HashMap<&'static str, Box<dyn crate::commands::Command>>,
}

impl Greiderbot {
    fn from_commands(commands: Vec<Box<dyn crate::commands::Command>>) -> Greiderbot {
        let commands = commands
            .into_iter()
            .map(|c| (c.name(), c))
            .collect::<HashMap<_, _>>();
        Greiderbot { commands }
    }
}

#[async_trait]
impl EventHandler for Greiderbot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        let guilds = join_all(ready.guilds.iter().map(|guild| async {
            match guild.id.to_partial_guild(&ctx.http).await {
                Ok(guild) => guild.name,
                Err(_) => guild.id.to_string(),
            }
        }))
        .await
        .join(", ");
        info!(
            "Hello from {} connected to guilds: {}",
            ready.user.name, guilds
        );
        info!("Registring commands");

        for guild in &ready.guilds {
            let commands = self.commands.values().map(|c| c.register()).collect();
            if let Err(why) = guild.id.set_commands(&ctx.http, commands).await {
                error!("Could not register commands in {}: {:?}", guild.id, why);
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        debug!("RECEIVED INTERACTION: {:?}", &interaction);
        match interaction {
            Interaction::Ping(_) => {}
            Interaction::Command(interaction) => {
                let Some(command) = self.commands.get(interaction.data.name.as_str()) else {
                    error!("Non existent command: {}", &interaction.data.name);
                    return;
                };
                command.run(interaction, &ctx).await;
            }
            Interaction::Component(interaction) => {
                let Some(command) = self.commands.get(interaction.data.custom_id.as_str()) else {
                    error!("Non existent: {:?}", &interaction);
                    return;
                };
                command.component_interaction(interaction, &ctx).await;
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let opt = Opt::parse();

    let discord_token =
        env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN environment variable");

    let log_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("{},hyper=info,mio=info", opt.log_level)));
    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    // Slash commands and components arrive without any gateway intents
    let mut client = Client::builder(&discord_token, GatewayIntents::empty())
        .event_handler(
            GreiderbotBuilder::default()
                .add_command(BlandaCommand)
                .add_command(RetireCommand::load(
                    env::var("PENSION_CATEGORIES_FILE")
                        .unwrap_or_else(|_| "pension_categories.json".to_string()),
                ))
                .add_command(SpelaCommand)
                .build(),
        )
        .await
        .expect("Could not create client");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
