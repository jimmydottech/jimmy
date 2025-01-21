mod actions;
mod attest;
mod client;
mod config;
mod constant;
mod feed;
mod jupiter;
mod llm;
mod pipeline;
mod portfolio;
mod price;
pub mod store;
mod strategy;
mod token;
mod twitter;
mod wallet;

use anyhow::Result;

use std::time::Duration;

use crate::attest::generate_raw_attestation;
use crate::config::Config;
use crate::constant::*;
use crate::feed::substack::SubstackFeed;
use crate::pipeline::PipelineBuilder;
use crate::token::jimmy::JimmyToken;
use crate::twitter::TwitterClient;
use crate::wallet::Wallet;

#[tokio::main]
async fn main() -> Result<()> {
    setup_env_and_tracing();

    let wallet = Wallet::get();

    let user_report = wallet.pubkey().to_bytes();
    let attestation = generate_raw_attestation(&user_report)?;
    tracing::info!("Generate attestation: {:?}", attestation);

    let pubkey = wallet.pubkey();
    let twitter_client = TwitterClient::get();
    if wallet.is_new() {
        let tweet = format!(
            "🚀 Hi, I'm Jimmy, fully autonomous AI agent! 🌍\n\nHere's my official wallet address:\n💼 {}\n\nTransparency and innovation, on-chain. Let's build the future together! 💡⚡\n\n#DeFi #AI #Blockchain",
            pubkey
        );
        twitter_client.post_tweet(&tweet).await?;
    } else {
        tracing::info!("Jimmy's wallet announce tweet already posted");
    }
    let profile_url = twitter_client.profile_url().await?;
    tracing::info!("Please check Jimmy's Twitter: {}", profile_url);

    let config = Config::get();
    let min_sol_balance_lamports = config.min_sol_balance_lamports();
    while wallet.balance()? < min_sol_balance_lamports {
        tracing::info!("Waiting for enough SOL...");
        tracing::info!(
            "Please transfer at least {} SOL to Jimmy's wallet: {}",
            config.min_sol_balance,
            wallet.pubkey()
        );

        // Airdrop SOL to Jimmy's wallet
        // Localnet / Devnet only
        if let Err(e) = wallet.get_airdrop(5 * LAMPORTS_PER_SOL).await {
            tracing::error!("Failed to get airdrop: {}", e);
        }

        tracing::info!("Waiting for 10 seconds...");
        tokio::time::sleep(Duration::from_secs(10)).await;
    }

    let balance = wallet.balance()?;
    tracing::info!("Current SOL balance: {} lamports", balance);

    let jimmy_token = JimmyToken::get().await;
    jimmy_token.print_metadata()?;
    jimmy_token.print_balance()?;

    let balance = wallet.balance()?;
    tracing::info!("Current SOL balance: {} lamports", balance);

    let pipeline = PipelineBuilder::new()
        .with_feed(SubstackFeed::new())
        // .with_feed(NewsletterFeed::new())
        .build();
    pipeline.run_loop().await?;

    Ok(())
}

pub fn setup_env_and_tracing() {
    dotenv::dotenv().ok();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}
