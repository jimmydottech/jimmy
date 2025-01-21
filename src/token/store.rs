use anyhow::Result;
use serde_json::Value;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::OnceLock;

use crate::store::map::StoreMap;
use crate::store::{LocalStore, Store};

use super::structs::TokenInfo;

pub struct SolanaTokenStore {
    tokens: StoreMap<String, TokenInfo, LocalStore>,
}

/*
[   {
        "address": "So11111111111111111111111111111111111111112",
        "name": "Wrapped SOL",
        "symbol": "SOL",
        "decimals": 9,
        "logoURI": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png",
        "tags": [
            "community",
            "strict",
            "verified"
        ],
        "daily_volume": 2351923373.903479,
        "created_at": "2024-04-26T10:56:58.893768Z",
        "freeze_authority": null,
        "mint_authority": null,
        "permanent_delegate": null,
        "minted_at": null,
        "extensions": {
            "coingeckoId": "wrapped-solana"
        }
    },
]
*/
impl SolanaTokenStore {
    const TOKENS_PREFIX: &'static str = "solana_tokens";
    pub fn get() -> &'static Self {
        static INSTANCE: OnceLock<SolanaTokenStore> = OnceLock::new();
        INSTANCE.get_or_init(|| SolanaTokenStore::new())
    }

    fn new() -> Self {
        Self {
            tokens: LocalStore::open_map(Self::TOKENS_PREFIX),
        }
    }

    pub fn tokens(&self) -> &StoreMap<String, TokenInfo, LocalStore> {
        &self.tokens
    }

    pub fn get_solana_symbol(&self, symbol: &str) -> String {
        match symbol {
            "BTC" => "WBTC".to_string(),
            "BONK" => "Bonk".to_string(),
            _ => symbol.to_string(),
        }
    }

    fn check_tokens_is_empty(&self) -> bool {
        if let None = self.tokens.iter().next() {
            true
        } else {
            false
        }
    }

    pub async fn get_token_info(&self, symbol: &str) -> Result<Option<TokenInfo>> {
        let need_update = self.check_tokens_is_empty();
        if need_update {
            self.update_tokens().await?;
        }

        let symbol = self.get_solana_symbol(symbol);
        self.tokens.get(&symbol)
    }

    async fn update_tokens(&self) -> Result<()> {
        // Jupiter token list endpoint
        let url = "https://tokens.jup.ag/tokens?tags=verified";
        let response = reqwest::get(url).await?;
        let json: Value = response.json().await?;

        if let Some(tokens) = json.as_array() {
            for token in tokens {
                let address = token
                    .get("address")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Pubkey::from_str(s).ok())
                    .ok_or(anyhow::anyhow!("Invalid address"))?;
                let decimals = token
                    .get("decimals")
                    .and_then(|v| v.as_u64())
                    .and_then(|d| u8::try_from(d).ok())
                    .ok_or(anyhow::anyhow!("Invalid decimals"))?;
                let name = token
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or(anyhow::anyhow!("Invalid name"))?;
                let symbol = token
                    .get("symbol")
                    .and_then(|v| v.as_str())
                    .ok_or(anyhow::anyhow!("Invalid symbol"))?;
                let coingecko_id = token
                    .get("extensions")
                    .and_then(|v| v.get("coingeckoId"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let token_info = TokenInfo {
                    address,
                    decimals,
                    name: name.to_string(),
                    symbol: symbol.to_string(),
                    coingecko_id: coingecko_id,
                };

                if let Err(e) = self.tokens.insert(symbol.to_string(), token_info) {
                    return Err(e);
                }
            }
        } else {
            return Err(anyhow::anyhow!("No tokens found"));
        }

        Ok(())
    }
}
