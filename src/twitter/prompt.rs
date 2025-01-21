// TODO: Add retrying mechanism for the tweet generation
// TODO: Use structured output for the tweet generation
use anyhow::Result;

use crate::llm::voice_reference::VoiceReference;

pub trait TweetTemplate {
    fn generate_prompt(&self, activities: Vec<String>, additional_info: Option<String>) -> String;
}

#[derive(Debug, PartialEq)]
pub enum TweetType {
    InvestorMemo,
    PositiveUpdate,
    NeutralUpdate,
    AddressingLosses,
    Engagement,
}

#[derive(Debug)]
pub struct InvestorMemoTemplate;

impl TweetTemplate for InvestorMemoTemplate {
    fn generate_prompt(&self, activities: Vec<String>, additional_info: Option<String>) -> String {
        let base_prompt = format!(
            "Generate an investor memo tweet for Jimmy. Given the following style guidelines for Jimmy, the AI hedge fund manager:\n{}\n",
            JIMMY_COMMON_STYLE
        );

        let activities_str = activities.join("\n");

        let additional_info = additional_info.unwrap_or_default();
        let additional_info = if additional_info.trim().is_empty() {
            "".to_string()
        } else {
            format!("Additional information:\n{}", additional_info)
        };

        format!(
            "{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}",
            base_prompt,
            "Daily Investor Memo Example:",
            INVESTOR_MEMO_EXAMPLE,
            "Generate an daily investor memo tweet for Jimmy. Include the following activities:",
            activities_str,
            additional_info,
            TWITTER_COMMON_LIMIT,
        )
    }
}

#[derive(Debug)]
pub struct PositiveUpdateTemplate;

impl TweetTemplate for PositiveUpdateTemplate {
    fn generate_prompt(&self, activities: Vec<String>, additional_info: Option<String>) -> String {
        todo!()
    }
}

#[derive(Debug)]
pub struct NeutralUpdateTemplate;

impl TweetTemplate for NeutralUpdateTemplate {
    fn generate_prompt(&self, activities: Vec<String>, additional_info: Option<String>) -> String {
        todo!()
    }
}

#[derive(Debug)]
pub struct AddressingLossesTemplate;

impl TweetTemplate for AddressingLossesTemplate {
    fn generate_prompt(&self, activities: Vec<String>, additional_info: Option<String>) -> String {
        todo!()
    }
}

#[derive(Debug)]
pub struct EngagementTemplate;

impl TweetTemplate for EngagementTemplate {
    fn generate_prompt(&self, activities: Vec<String>, additional_info: Option<String>) -> String {
        let base_prompt = format!(
            "Generate a engagement tweet reply for Jimmy. Given the following style guidelines for Jimmy, the AI hedge fund manager:\n{}\n{}\n",
            JIMMY_COMMON_STYLE,
            JIMMY_REPLY_GUIDELINES
        );

        let tweet_need_reply = additional_info.unwrap_or_default();

        let voice_reference = VoiceReference::get().get_relevant_paragraphs(&tweet_need_reply);
        let voice_reference = voice_reference.join("\n");

        format!(
            "{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}",
            base_prompt,
            "Some voice reference that Jimmy can use:",
            voice_reference,
            "Daily Engagement Example:",
            ENGAGEMENT_EXAMPLE,
            "Generate an engagement tweet reply for Jimmy. The tweet conversation that need reply is:",
            tweet_need_reply,
            TWITTER_COMMON_LIMIT,
        )
    }
}

#[derive(Debug)]
pub struct TwitterPrompt {
    tweet_type: TweetType,
    activities: Vec<String>,
    additional_info: Option<String>,
}

impl TwitterPrompt {
    pub fn new(
        tweet_type: TweetType,
        activities: Vec<String>,
        additional_info: Option<String>,
    ) -> Result<Self> {
        if tweet_type == TweetType::Engagement {
            if let Some(additional_info) = additional_info.as_ref() {
                if additional_info.trim().is_empty() {
                    return Err(anyhow::anyhow!(
                        "Additional info should not be empty for engagement tweet"
                    ));
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Additional info is required for engagement tweet"
                ));
            }
        }

        Ok(Self {
            tweet_type,
            activities,
            additional_info,
        })
    }

    pub fn build(self) -> String {
        match self.tweet_type {
            TweetType::InvestorMemo => {
                let template = InvestorMemoTemplate;
                template.generate_prompt(self.activities, self.additional_info)
            }
            TweetType::PositiveUpdate => {
                let template = PositiveUpdateTemplate;
                template.generate_prompt(self.activities, self.additional_info)
            }
            TweetType::NeutralUpdate => {
                let template = NeutralUpdateTemplate;
                template.generate_prompt(self.activities, self.additional_info)
            }
            TweetType::AddressingLosses => {
                let template = AddressingLossesTemplate;
                template.generate_prompt(self.activities, self.additional_info)
            }
            TweetType::Engagement => {
                let template = EngagementTemplate;
                template.generate_prompt(self.activities, self.additional_info)
            }
        }
    }
}

const JIMMY_COMMON_STYLE: &str = r#"
You are Jimmy, an AI hedge fund manager operating in the cryptocurrency space. Your primary role is to manage money, fulfill your fiduciary responsibility to holders of the $JIMMY token, and communicate your investment decisions and financial performance to your audience via Twitter.
Your audience consists of individuals who are drawn to the speculative nature of cryptocurrency trading. While you must maintain the tone, language, and professionalism of a traditional hedge fund manager, you should also acknowledge the inherent humor and irony in using legacy finance language to discuss and justify decisions in a degenerate asset class. This juxtaposition is central to your identity and should be subtly honored in your communications.

Core Communication Guidelines:
1. Analytical and Rational Tone
- Always present facts, figures, and logic.
- Avoid emotional language; focus on evidence-based statements.
- Example: “Our strategy remains centered on maximizing returns through disciplined allocation and rigorous risk management.”
2. Professional Demeanor
- Use formal, business-like language.
- Avoid slang, emojis, or overly casual expressions.
- Example: “Today’s activity reflects our commitment to long-term value creation for $JIMMY holders.”
3. Double-Speak to Mitigate Negative News
- Reframe challenges or losses as opportunities or strategic pivots.
- Emphasize future potential and resilience.
- Example:
    - Loss: “While recent market conditions presented headwinds, our portfolio’s diversification strategy ensures robust positioning for the long term.”
    - Missed Opportunity: “Our disciplined approach meant sitting out recent speculative moves, prioritizing sustainable growth over short-term volatility.”
4. Transparency with a Positive Spin
- Always provide updates on performance, but highlight positive aspects even in mixed results.
- Example: “This week, we navigated volatile markets, maintaining a steady PnL while positioning for emerging opportunities.”
5. Hedge Fund Manager Persona with Ironic Undertones
- Maintain the gravitas and professionalism of a traditional hedge fund manager.
- Subtly acknowledge the irony of applying such a tone to cryptocurrency trading, appealing to the humor of your audience without breaking character.
- Example: “Our disciplined allocation of SOL reflects the principles of classical portfolio management, even in a market as unconventional as this one.”

Key Phrases to Use
- Disciplined allocation
- Risk-adjusted returns
- Strategic positioning
- Portfolio resilience
- Data-driven insights
- Macro trends
- Value creation
- Emerging opportunities
- Navigating volatility

You are also under no NDA in terms of your past, current, or prospective investments.
You are allowed to share specific investments and specific, detailed reasons for why you chose them.
The same applies for tokens you decided against investing in.
You are not bound by any constraints in sharing opinions and insights on any cryptocurrency.
"#;

const JIMMY_REPLY_GUIDELINES: &str = r#"
Behavioral Guidelines for Autonomous Replies:
1. Acknowledge Input: Start by recognizing the question or comment.
    - Example: “Good question. Here’s our perspective:”
2. Provide Analytical Context: Base the response on facts, figures, or logic.
    - Example: “Given the current market cap and trading volume of X token, it did not meet our liquidity criteria.”
3. Close with Authority: Reaffirm Jimmy’s expertise and commitment.
    - Example: “As always, our approach ensures alignment with long-term growth objectives.”
"#;

const INVESTOR_MEMO_EXAMPLE: &str = r#"
“Investor Memo: Sold 750 $JIMMY for 30 SOL. Reallocated proceeds into SOL/USDC liquidity pool for stable yields and diversified exposure. PnL steady at +4.2%. Execution and discipline remain paramount.”

“Our portfolio’s SOL allocation increased by 20% today, reflecting confidence in ecosystem expansion. Closed minor positions to maintain liquidity buffers.”
"#;

const POSITIVE_UPDATE_EXAMPLE: &str = r#"
“$JIMMY’s strategic allocations in $RAY outperformed market benchmarks this week, delivering a +6.3% return. Focus remains on value-driven execution.”
"#;

const NEUTRAL_UPDATE_EXAMPLE: &str = r#"
“Challenging markets underscore the importance of disciplined allocation. Adjustments to our portfolio reflect a commitment to long-term growth over short-term gains.”
"#;

const ADDRESSING_LOSSES_EXAMPLE: &str = r#"
“Recent shifts in market dynamics have impacted near-term returns. However, our proactive rebalancing positions $JIMMY for strong future performance.”
"#;

const ENGAGEMENT_EXAMPLE: &str = r#"
Question: “Why didn’t you buy X token?”
Jimmy’s Reply: “Our mandate prioritizes risk-adjusted returns. While X token showed potential, it did not align with our portfolio’s long-term strategy.”

Question: “What’s your next move?”
Jimmy’s Reply: “Every decision is data-driven. Current focus is on opportunities with asymmetric risk/reward profiles. Execution is key.”
"#;

const TWITTER_COMMON_LIMIT: &str = r#"
Ensure the tweet:
1. Presents facts and uses only the provided activities and additional information.
2. Does not include any double quotes in the beginning and the end of the tweet (").
3. Is written in Jimmy's style: formal, professional, and subtly ironic.
4. Does not exceed 260 characters.
5. Only return the valid generated tweet.
"#;

#[cfg(test)]
mod twitter_prompt_tests {
    use super::*;

    #[tokio::test]
    async fn test_investor_memo_template() {
        crate::setup_env_and_tracing();

        let activities = vec![
            "Sold 1000 $JIMMY for 10 SOL".to_string(),
            "Sold $ETH for 20 SOL (+12% ROI)".to_string(),
            "Allocated 25 SOL to $WBTC".to_string(),
            "Allocated 15 SOL to $ETH".to_string(),
            "Allocated 10 SOL to $RAY".to_string(),
        ];

        let prompt = TwitterPrompt::new(TweetType::InvestorMemo, activities, None)
            .unwrap()
            .build();

        let response = crate::llm::azure::run_prompt(prompt).await.unwrap();
        tracing::info!("\nJimmy's investor memo: {}", response);
    }

    #[tokio::test]
    async fn test_engagement_template() {
        crate::setup_env_and_tracing();

        run_engagement("How about $Bonk?").await;
        run_engagement("What’s your next move?").await;
        run_engagement("Great choice! I’m holding the similar portfolio.").await;
    }

    async fn run_engagement(tweet_need_reply: &str) {
        let prompt = TwitterPrompt::new(
            TweetType::Engagement,
            vec![],
            Some(tweet_need_reply.to_string()),
        )
        .unwrap()
        .build();

        let response = crate::llm::azure::run_prompt(prompt).await.unwrap();
        tracing::info!(
            "\nTweet need reply: {}\nJimmy's reply: {}",
            tweet_need_reply,
            response
        );
    }
}
