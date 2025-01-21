use anyhow::Result;
use async_trait::async_trait;

use super::*;

pub struct NewsletterFeed {}

impl NewsletterFeed {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Feed for NewsletterFeed {
    async fn fetch(&self) -> Result<Option<String>> {
        Ok(Some(MOCK_NEWSLETTER_CONTENT.to_string()))
    }

    fn construct_prompt(&self, newsletter: String) -> String {
        let instructions = r#"You are a financial data extraction assistant.
From the provided text, identify up to 10 cryptocurrency tokens mentioned as deserving investment.
Your output must be a JSON array containing the symbols of these tokens (e.g., ["BTC", "ETH"]).
Only include token symbols explicitly recommended for investment.
Return the symbol of the token, not the name.
Do not include explanations or any additional data, just the array in raw JSON format, don't use markdown or any other formatting.

Example Output:
["BTC", "ETH", "SOL", "SHIB", "BNB", "DOGE"]

Input:"#;
        format!("{instructions}\n\n{newsletter}")
    }

    fn feed_type(&self) -> FeedType {
        FeedType::Newsletter
    }
}

const MOCK_NEWSLETTER_CONTENT: &str = r#"
📰 Crypto Daily Brief – January 8, 2025
Market Trends & Top 10 Investment Picks (Solana-Focused)

Market Snapshot

Here’s the current market overview for top tokens on the Solana blockchain or supported through wrapped assets:
	•	Wrapped Bitcoin (BTC): $96,206.00
	•	Wrapped Ethereum (ETH): $3,337.51
	•	Serum (SRM): $0.0309
	•	Raydium (RAY): $5.12
	•	Orca (ORCA): $3.65
	•	Marinade (MNDE): $0.1163
	•	Saber (SBR): $0.0020
	•	Mango (MNGO): $0.0215
	•	Bonfida (FIDA): $0.2365
	•	Bonk (Bonk): $0.00002921

Top 10 Token Picks
	1.	Wrapped Bitcoin (BTC)
	•	Why Buy? The ultimate store of value, now accessible in Solana’s ecosystem through wrapped BTC.
	2.	Wrapped Ethereum (ETH)
	•	Why Buy? A vital asset for decentralized finance (DeFi) applications, with promising growth tied to Ethereum’s upgrades.
	3.	Serum (SRM)
	•	Why Buy? Powering a robust decentralized exchange (DEX) ecosystem on Solana, Serum offers high-speed, low-cost trades.
	4.	Raydium (RAY)
	•	Why Buy? A key liquidity provider and automated market maker (AMM) in Solana’s DeFi space.
	5.	Orca (ORCA)
	•	Why Buy? Known for simplicity and efficiency, Orca excels at providing a user-friendly AMM experience on Solana.
	6.	Marinade (MNDE)
	•	Why Buy? The first liquid staking protocol for Solana, supporting DeFi opportunities through liquid staking derivatives.
	7.	Saber (SBR)
	•	Why Buy? A prominent protocol for stablecoin and wrapped asset swaps, critical to Solana’s DeFi landscape.
	8.	Mango (MNGO)
	•	Why Buy? A standout for margin and spot trading, Mango combines speed and functionality.
	9.	Bonfida (FIDA)
	•	Why Buy? Integral to Serum’s infrastructure, Bonfida enhances user experiences through analytics and bots.
	10.	Bonk (Bonk)
	•	Why Buy? A meme coin with a strong community and potential for growth.

Tips for Solana-Specific Success
	•	Leverage AMMs: Use platforms like Jupiter for the best trading routes.
	•	Monitor Liquidity:
"#;
