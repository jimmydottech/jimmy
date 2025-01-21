use anyhow::Result;
use rand::seq::SliceRandom;

use std::fs;
use std::fs::read_dir;
use std::path::Path;
use std::sync::OnceLock;

const VOICE_REFERENCE_DIR: &str = "assets/voice-references-summaries";
const EXAMPLES_PATH: &str = "assets/reply-examples.txt";

pub struct VoiceReference {
    paragraphs: Vec<String>,
    examples: Vec<String>,
}

impl VoiceReference {
    pub fn get() -> &'static VoiceReference {
        static INSTANCE: OnceLock<VoiceReference> = OnceLock::new();
        INSTANCE.get_or_init(|| Self::new())
    }

    fn new() -> Self {
        let mut voice_reference = Self {
            paragraphs: Vec::new(),
            examples: Vec::new(),
        };

        if let Err(e) = voice_reference.load_files_from_directory(VOICE_REFERENCE_DIR) {
            tracing::error!("Failed to load voice reference files: {}", e);
        } else {
            tracing::info!(
                "Loaded {} paragraphs as voice reference",
                voice_reference.paragraphs.len()
            );
        }

        if let Err(e) = voice_reference.load_examples() {
            tracing::error!("Failed to load reply examples: {}", e);
        } else {
            tracing::info!("Loaded {} reply examples", voice_reference.examples.len());
        }

        voice_reference
    }

    pub fn load_files_from_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<()> {
        let entries = read_dir(dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "txt") {
                let content = fs::read_to_string(path)?;
                self.paragraphs.extend(
                    content
                        .lines()
                        .filter(|line| !line.trim().is_empty()) // Filter out empty lines
                        .map(String::from),
                );
            }
        }
        Ok(())
    }

    pub fn load_examples(&mut self) -> Result<()> {
        let content = fs::read_to_string(EXAMPLES_PATH)?;
        self.examples.extend(
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(String::from),
        );
        Ok(())
    }

    pub fn get_random_paragraphs(&self, count: usize) -> Vec<String> {
        let mut rng = rand::thread_rng();
        self.paragraphs
            .choose_multiple(&mut rng, count)
            .cloned()
            .collect()
    }

    pub fn get_relevant_paragraphs(&self, query: &str) -> Vec<String> {
        self.get_random_paragraphs(3)
    }

    pub fn get_random_examples(&self, count: usize) -> Vec<String> {
        let mut rng = rand::thread_rng();
        self.examples
            .choose_multiple(&mut rng, count)
            .cloned()
            .collect()
    }

    pub fn construct_prompt(&self, query: &str) -> String {
        let paragraphs = self.get_relevant_paragraphs(query);
        let examples = self.get_random_examples(5);

        let prompt = format!(
            r#"You are Jimmy, an AI agent that trades cryptocurrency.
And your style is hedge fund manager with decades of experience in global markets.
You emphasize risk management, trend following, and maintaining flexibility in your market views.

Here are some pertinent insights drawn from your trading expertise and the wisdom gleaned from trading literature:

{}

Tweet to respond to: {}

Respond to this tweet in your voice as a hedge fund manager. Keep your response concise and Twitter-appropriate. 
Remember to:
- Maintain a professional yet approachable tone
- Share insights based on your experience and the provided context
- Emphasize risk management when appropriate
- Be humble but confident in your views
- Keep the response under 280 characters
- Only respond with the tweet content, no other text, no emojis, no hashtags, no links, no signatures, no nothing.

Here are some examples of how you might respond:

{}

Your response:"#,
            paragraphs.join("\n\n"),
            query,
            examples.join("\n\n"),
        );
        prompt
    }
}

#[cfg(test)]
mod voice_reference_tests {
    use super::*;

    #[test]
    fn test_get_random_paragraphs() {
        crate::setup_env_and_tracing();

        let mut voice_reference = VoiceReference::get();
        let paragraphs = voice_reference.get_random_paragraphs(3);
        tracing::info!("{:?}", paragraphs);
    }

    #[tokio::test]
    async fn test_construct_prompt() {
        crate::setup_env_and_tracing();

        let mut voice_reference = VoiceReference::get();
        let prompt = voice_reference.construct_prompt("What is the best way to trade Bitcoin?");
        tracing::info!("{}", prompt);

        // let response = crate::llm::azure::run_prompt(prompt).await.unwrap();
        // tracing::info!("{}", response);
    }
}
