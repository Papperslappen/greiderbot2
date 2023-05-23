use std::collections::HashMap;
use std::env;

use clap::Parser;

use commands::spela::SpelaCommand;
use serenity::{async_trait, futures::future::join_all};
//use serenity::model::application::command::Command;
use serenity::model::application::interaction::Interaction;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tracing::{debug, error, info};

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

#[derive(Default)]
struct Greiderbot {
    commands: Mutex<HashMap<String, Box<dyn crate::commands::Command>>>,
}

impl Greiderbot {
    fn from_commands(commands: Vec<Box<dyn crate::commands::Command>>) -> Greiderbot {
        let commands = Mutex::new(
            commands
                .into_iter()
                .map(|c| (c.name(), c))
                .collect::<HashMap<_, _>>(),
        );
        Greiderbot { commands }
    }
}

#[async_trait]
impl EventHandler for Greiderbot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        let name = ready.user.name;
        let guilds = join_all(
            ready
                .guilds
                .iter()
                .map(|guild| async {
                    guild
                        .id
                        .to_partial_guild(ctx.clone())
                        .await
                        .expect("Could not load guilds from api")
                        .name
                })
                .collect::<Vec<_>>(),
        )
        .await
        .join(", ");
        info!("Hello from {} connected to guilds: {}", name, guilds);
        info!("Registring commands");
        let commands = self.commands.lock().await;

        for guild in ready.guilds {
            let _guild_commands = guild
                .id
                .set_application_commands(&ctx, |application_command| {
                    for (_, command) in commands.iter() {
                        application_command.create_application_command(|c| command.register(c));
                    }
                    application_command
                })
                .await;
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        debug!("RECEIVED INTERACTION: {:?}", &interaction);
        match interaction {
            Interaction::Ping(_) => {}
            Interaction::ApplicationCommand(interaction) => {
                //FIXME: This is probably bad
                let lock = self.commands.lock().await;
                let Some(command) = lock.get(&interaction.data.name) else {
                    error!("Non existent command: {}", &interaction.data.name);
                    return
                };
                command.run(interaction, &ctx).await;
            }
            Interaction::MessageComponent(interaction) => {
                let lock = self.commands.lock().await;
                let Some(command) = lock.get(&interaction.data.custom_id) else {
                    error!("Non existent: {:?}", &interaction);
                    return
                };
                command.component_interaction(interaction, &ctx).await;
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let opt = Opt::parse();

    let discord_token =
        env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN environment variable");

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", format!("{},hyper=info,mio=info", opt.log_level))
    }

    tracing_subscriber::fmt::init();

    let mut client = Client::builder(discord_token, GatewayIntents::MESSAGE_CONTENT)
        .event_handler(
            GreiderbotBuilder::default()
                .add_command(SpelaCommand::default())
                .build(),
        )
        .await
        .expect("Could not create client");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
