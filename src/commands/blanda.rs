use rand::seq::SliceRandom;
use serenity::{
    async_trait,
    builder::{
        CreateAllowedMentions, CreateCommand, CreateCommandOption, CreateInteractionResponse,
        CreateInteractionResponseMessage,
    },
    client::Context,
    model::application::{CommandInteraction, CommandOptionType},
};
use tracing::error;

use super::Command;

const WORDS_OPTION: &str = "words";

#[derive(Default)]
pub(crate) struct BlandaCommand;

#[async_trait]
impl Command for BlandaCommand {
    fn name(&self) -> &'static str {
        "blanda"
    }

    fn register(&self) -> CreateCommand {
        CreateCommand::new(self.name())
            .name_localized("sv-SE", "blanda")
            .description("Mix a list of words")
            .description_localized("sv-SE", "Blanda en lista med ord")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    WORDS_OPTION,
                    "Words separated by spaces, or by commas",
                )
                .name_localized("sv-SE", "ord")
                .description_localized("sv-SE", "Ord separerade med mellanslag, eller med komma")
                .required(true)
                // Leaves room for the separators within the 2000 character message limit
                .max_length(1000),
            )
    }

    async fn run(&self, command: CommandInteraction, ctx: &Context) {
        let Some(input) = command
            .data
            .options
            .iter()
            .find(|option| option.name == WORDS_OPTION)
            .and_then(|option| option.value.as_str())
        else {
            error!("Missing option: {}", WORDS_OPTION);
            return;
        };

        let content = shuffle_words(input);
        let message = if content.is_empty() {
            CreateInteractionResponseMessage::new()
                .content("Det finns inga ord att blanda")
                .ephemeral(true)
        } else {
            CreateInteractionResponseMessage::new()
                .content(content)
                // The words are user input and must not ping anyone
                .allowed_mentions(CreateAllowedMentions::new())
        };

        if let Err(why) = command
            .create_response(&ctx.http, CreateInteractionResponse::Message(message))
            .await
        {
            error!("Could not reply to command: {:?}", why);
        }
    }
}

/// Splits on commas if there are any, so that things can consist of several words,
/// and on whitespace otherwise.
fn shuffle_words(input: &str) -> String {
    let (mut words, separator) = if input.contains(',') {
        (input.split(',').map(str::trim).collect::<Vec<_>>(), ", ")
    } else {
        (input.split_whitespace().collect::<Vec<_>>(), " ")
    };
    words.retain(|word| !word.is_empty());
    words.shuffle(&mut rand::rng());
    words.join(separator)
}

#[cfg(test)]
mod tests {
    use super::shuffle_words;

    fn sorted(s: &str, separator: &str) -> Vec<String> {
        let mut words = s.split(separator).map(String::from).collect::<Vec<_>>();
        words.sort();
        words
    }

    #[test]
    fn keeps_the_same_words() {
        assert_eq!(
            sorted(&shuffle_words("a  b c\td"), " "),
            ["a", "b", "c", "d"]
        );
    }

    #[test]
    fn commas_keep_multi_word_things_together() {
        assert_eq!(
            sorted(&shuffle_words("Kalle Anka,Musse Pigg, , Långben"), ", "),
            ["Kalle Anka", "Långben", "Musse Pigg"]
        );
    }

    #[test]
    fn empty_input() {
        assert_eq!(shuffle_words(" , ,"), "");
    }
}
