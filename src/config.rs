use serde::{Deserialize, Serialize};

use std::sync::OnceLock;

use crate::constant::*;
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    // Network configuration
    pub solana_rpc_url: String,

    // Wallet configuration
    pub min_sol_balance: f64, // in SOL

    // Token configuration
    pub token_name: String,
    pub token_symbol: String,
    pub token_uri: String,
    pub token_decimals: u8,
    pub total_supply: u64, // total number of tokens to mint

    // Raydium
    pub raydium_pool_price: f64,
    pub raydium_pool_min_price: f64,
    pub raydium_pool_max_price: f64,
    pub raydium_pool_deposit: f64,

    // Azure OpenAI configuration
    pub azure_openai_api_key: String,
    pub azure_openai_endpoint: String,
    pub azure_openai_api_version: String,
    pub azure_openai_chat_model: String,

    // Price API configuration
    pub coingecko_api_key: Option<String>,
    pub coinmarketcap_api_key: Option<String>,

    // Jupiter configuration
    pub jupiter_rpc_url: String,

    // Twitter configuration
    pub use_twitter: bool,
    pub twitter_consumer_key: String,
    pub twitter_consumer_key_secret: String,
    pub twitter_access_token: String,
    pub twitter_access_token_secret: String,

    // Mock trade configuration
    pub mock_trade: bool,
    pub sell_jimmy_amount: f64,
    pub max_sol_trading_amount_one_day: f64,
    pub min_profit_rate: f64,

    // Substack configuration
    pub substack_urls: Vec<String>,

    // store path
    pub store_path: String,

    // TEE env
    pub run_in_sgx: bool,
}

impl Config {
    pub fn get() -> &'static Config {
        static INSTANCE: OnceLock<Config> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let solana_rpc_url =
                std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8899".into());
            let min_sol_balance = std::env::var("MIN_SOL_BALANCE")
                .unwrap_or_else(|_| "1.0".into())
                .parse()
                .expect("MIN_SOL_BALANCE must be a valid float");
            let token_name = std::env::var("TOKEN_NAME").unwrap_or_else(|_| "Jimmy Token".into());
            let token_symbol = std::env::var("TOKEN_SYMBOL").unwrap_or_else(|_| "JIMMY".into());
            let token_uri = std::env::var("TOKEN_URI").unwrap_or_else(|_| "".into());
            let token_decimals = std::env::var("TOKEN_DECIMALS")
                .unwrap_or_else(|_| "9".into())
                .parse()
                .expect("TOKEN_DECIMALS must be a valid u8");
            let total_supply = std::env::var("TOTAL_SUPPLY")
                .unwrap_or_else(|_| "1000000000".into())
                .parse()
                .expect("TOTAL_SUPPLY must be a valid u64");

            let raydium_pool_price = std::env::var("RAYDIUM_POOL_PRICE")
                .expect("RAYDIUM_POOL_PRICE is not set")
                .parse()
                .expect("RAYDIUM_POOL_PRICE must be a valid f64");
            let mut raydium_pool_min_price = std::env::var("RAYDIUM_POOL_MIN_PRICE")
                .expect("RAYDIUM_POOL_MIN_PRICE is not set")
                .parse()
                .expect("RAYDIUM_POOL_MIN_PRICE must be a valid f64");
            let mut raydium_pool_max_price = std::env::var("RAYDIUM_POOL_MAX_PRICE")
                .expect("RAYDIUM_POOL_MAX_PRICE is not set")
                .parse()
                .expect("RAYDIUM_POOL_MAX_PRICE must be a valid f64");
            let raydium_pool_deposit = std::env::var("RAYDIUM_POOL_DEPOSIT")
                .expect("RAYDIUM_POOL_DEPOSIT is not set")
                .parse()
                .expect("RAYDIUM_POOL_DEPOSIT must be a valid f64");
            if raydium_pool_min_price > raydium_pool_max_price {
                std::mem::swap(&mut raydium_pool_min_price, &mut raydium_pool_max_price);
            }

            let azure_openai_api_key =
                std::env::var("AZURE_OPENAI_API_KEY").expect("AZURE_OPENAI_API_KEY is not set");
            let azure_openai_endpoint =
                std::env::var("AZURE_OPENAI_ENDPOINT").expect("AZURE_OPENAI_ENDPOINT is not set");
            let azure_openai_api_version = std::env::var("AZURE_OPENAI_API_VERSION")
                .expect("AZURE_OPENAI_API_VERSION is not set");
            let azure_openai_chat_model = std::env::var("AZURE_OPENAI_CHAT_MODEL")
                .expect("AZURE_OPENAI_CHAT_MODEL is not set");

            let coingecko_api_key = std::env::var("COINGECKO_API_KEY").ok();
            let coinmarketcap_api_key = std::env::var("COINMARKETCAP_API_KEY").ok();

            let jupiter_rpc_url =
                std::env::var("JUPITER_RPC_URL").expect("JUPITER_RPC_URL is not set");

            let use_twitter = std::env::var("USE_TWITTER")
                .unwrap_or_else(|_| "false".into())
                .parse()
                .expect("USE_TWITTER must be a valid boolean");
            let twitter_consumer_key = std::env::var("TWITTER_CONSUMER_KEY").unwrap_or_default();
            let twitter_consumer_key_secret =
                std::env::var("TWITTER_CONSUMER_KEY_SECRET").unwrap_or_default();
            let twitter_access_token = std::env::var("TWITTER_ACCESS_TOKEN").unwrap_or_default();
            let twitter_access_token_secret =
                std::env::var("TWITTER_ACCESS_TOKEN_SECRET").unwrap_or_default();

            let mock_trade = std::env::var("MOCK_TRADE")
                .unwrap_or_else(|_| "false".into())
                .parse()
                .expect("MOCK_TRADE must be a valid boolean");
            let sell_jimmy_amount = std::env::var("SELL_JIMMY_AMOUNT")
                .expect("SELL_JIMMY_AMOUNT is not set")
                .parse()
                .expect("SELL_JIMMY_AMOUNT must be a valid f64");
            let max_sol_trading_amount_one_day = std::env::var("MAX_SOL_TRADING_AMOUNT_ONE_DAY")
                .expect("MAX_SOL_TRADING_AMOUNT_ONE_DAY is not set")
                .parse()
                .expect("MAX_SOL_TRADING_AMOUNT_ONE_DAY must be a valid f64");
            let min_profit_rate = std::env::var("MIN_PROFIT_RATE")
                .expect("MIN_PROFIT_RATE is not set")
                .parse()
                .expect("MIN_PROFIT_RATE must be a valid f64");

            let substack_urls = std::env::var("SUBSTACK_SUBSCRIPTION_URLS")
                .map(|urls| urls.split(",").map(|s| s.to_string()).collect())
                .unwrap_or(vec![]);

            let store_path = std::env::var("STORE_PATH").unwrap_or_else(|_| "store".into());

            let run_in_sgx = {
                let sgx = std::env::var("SGX").unwrap_or_else(|_| "false".into());
                sgx == "1" || sgx == "true" || sgx == "True"
            };

            Config {
                solana_rpc_url,
                min_sol_balance,
                token_name,
                token_symbol,
                token_uri,
                token_decimals,
                total_supply,
                raydium_pool_price,
                raydium_pool_min_price,
                raydium_pool_max_price,
                raydium_pool_deposit,
                azure_openai_api_key,
                azure_openai_endpoint,
                azure_openai_api_version,
                azure_openai_chat_model,
                coingecko_api_key,
                coinmarketcap_api_key,
                jupiter_rpc_url,
                use_twitter,
                twitter_consumer_key,
                twitter_consumer_key_secret,
                twitter_access_token,
                twitter_access_token_secret,
                mock_trade,
                sell_jimmy_amount,
                max_sol_trading_amount_one_day,
                min_profit_rate,
                substack_urls,
                store_path,
                run_in_sgx,
            }
        })
    }

    pub fn min_sol_balance_lamports(&self) -> u64 {
        (self.min_sol_balance * LAMPORTS_PER_SOL as f64) as u64
    }
}
