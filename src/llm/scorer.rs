use anyhow::Result;

use crate::llm::azure::run_prompt;

pub async fn score_reply(tweet: &str) -> Result<u8> {
    let prompt = format!(
        "Rate this Twitter reply on a scale of 1-10 based on how worthwhile it is to respond:

    \"{tweet}\"

    Scoring guidelines:
    0-2: Do not respond
    - Spam, bots, or promotional content
    - Hostile or toxic messages
    - Simple yes/no answers
    - Generic greetings or emojis only

    3-5: Consider responding
    - Basic questions or comments
    - Shows mild interest in discussion
    - Standard opinions or observations
    - Polite but unremarkable interactions

    6-8: Good to respond
    - Thoughtful questions or insights
    - Shows genuine curiosity
    - Potential for meaningful dialogue
    - Relevant to your expertise/interests

    9-10: Priority response
    - Highly engaging discussion points
    - Unique perspectives or ideas
    - Perfect opportunity for valuable input
    - Likely to generate quality interaction

    Provide only the numerical score (1-10) as your response and NOTHING ELSE."
    );

    let response = run_prompt(prompt).await?;
    let score = response.parse::<u8>()?;
    Ok(score)
}
