use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::config::Config;

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoricalPriceResponse {
    pub prices: Vec<Vec<f64>>,        // Each entry is [timestamp, price]
    pub market_caps: Vec<Vec<f64>>,   // Each entry is [timestamp, market cap]
    pub total_volumes: Vec<Vec<f64>>, // Each entry is [timestamp, total volume]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Coin {
    pub id: String,
    pub symbol: String,
    pub name: String,
}

pub struct CoinGeckoProvider {
    client: Client,
    api_key: String,
    coins: RwLock<HashMap<String, Coin>>,
}

impl CoinGeckoProvider {
    pub fn get() -> &'static Self {
        static INSTANCE: OnceLock<CoinGeckoProvider> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let config = Config::get();
            let api_key = config
                .coingecko_api_key
                .clone()
                .expect("Coingecko API key not found");
            CoinGeckoProvider::new(api_key)
        })
    }

    pub fn new(api_key: String) -> Self {
        let client = Client::new();
        let coins = RwLock::new(HashMap::new());

        CoinGeckoProvider {
            client,
            api_key,
            coins,
        }
    }

    pub async fn get_id_by_name(&self, name: &str) -> Result<String> {
        let need_init = self.coins.read().await.is_empty();
        if need_init {
            self.update_coins_list().await?;
        }

        let coin_id = self
            .coins
            .read()
            .await
            .get(name)
            .ok_or(anyhow::anyhow!("Coin not found"))?
            .id
            .clone();
        Ok(coin_id)
    }

    pub async fn get_coins_list(&self) -> Result<Vec<Coin>> {
        let url = "https://api.coingecko.com/api/v3/coins/list";
        let response = self
            .client
            .get(url)
            .header("x-cg-demo-api-key", &self.api_key)
            .send()
            .await?;
        let json: Value = response.json().await?;
        let coins: Vec<Coin> = serde_json::from_value(json)?;

        Ok(coins)
    }

    pub async fn update_coins_list(&self) -> Result<()> {
        let coins = self.get_coins_list().await?;
        let mut coins_map = self.coins.write().await;
        coins_map.clear();
        for coin in coins {
            coins_map.insert(coin.name.clone(), coin);
        }
        Ok(())
    }

    /// Get historical price of a coin
    ///
    /// # Arguments
    ///
    /// * `coin_name` - The name of the coin
    /// * `vs_currency` - The currency to convert the price to, e.g. "usd"
    /// * `days` - The number of days from current time to get the historical price for
    ///
    /// 1 day from current time = 5-minutely data
    /// 2 - 90 days from current time = hourly data
    /// above 90 days from current time = daily data (00:00 UTC)
    pub async fn get_historical_price_by_name(
        &self,
        coin_name: &str,
        vs_currency: &str,
        days: u32,
    ) -> Result<HistoricalPriceResponse> {
        let coin_id = self.get_id_by_name(coin_name).await?;

        self.get_historical_price_by_id(&coin_id, vs_currency, days)
            .await
    }

    pub async fn get_historical_price_by_id(
        &self,
        coin_id: &str,
        vs_currency: &str,
        days: u32,
    ) -> Result<HistoricalPriceResponse> {
        let url = format!(
            "https://api.coingecko.com/api/v3/coins/{}/market_chart?vs_currency={}&days={}",
            coin_id, vs_currency, days
        );

        let response = self
            .client
            .get(&url)
            .header("x-cg-demo-api-key", &self.api_key) // Use your API key here
            .send()
            .await?
            .json::<HistoricalPriceResponse>()
            .await?;

        Ok(response)
    }

    pub async fn get_price_by_id(&self, coin_id: &str, vs_currency: &str) -> Result<f64> {
        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}",
            coin_id, vs_currency
        );

        let response = self
            .client
            .get(&url)
            .header("x-cg-demo-api-key", &self.api_key)
            .send()
            .await?;
        let json: Value = response.json().await?;

        let price = json
            .get(coin_id)
            .and_then(|v| v.get(vs_currency))
            .and_then(|v| v.as_f64())
            .ok_or(anyhow::anyhow!("Failed to get price"))?;

        Ok(price)
    }

    pub async fn get_prices_by_ids(
        &self,
        coin_ids: &[&str],
        vs_currency: &str,
    ) -> Result<Vec<f64>> {
        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}",
            coin_ids.join(","),
            vs_currency
        );

        let response = self
            .client
            .get(&url)
            .header("x-cg-demo-api-key", &self.api_key)
            .send()
            .await?;
        let json: Value = response.json().await?;

        let mut prices = vec![];
        for coin_id in coin_ids {
            let price = json
                .get(coin_id)
                .and_then(|v| v.get(vs_currency))
                .and_then(|v| v.as_f64())
                .ok_or(anyhow::anyhow!("Failed to get price"))?;
            prices.push(price);
        }

        Ok(prices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_coins_list() {
        crate::setup_env_and_tracing();

        let coingecko = CoinGeckoProvider::get();
        coingecko.update_coins_list().await.unwrap();
    }

    #[tokio::test]
    async fn test_get_historical_price() {
        crate::setup_env_and_tracing();

        let coingecko = CoinGeckoProvider::get();
        let historical_price = coingecko
            .get_historical_price_by_name("Bitcoin", "usd", 3)
            .await
            .unwrap();

        tracing::info!("{:?}", historical_price.prices);
    }

    #[tokio::test]
    async fn test_get_price() {
        crate::setup_env_and_tracing();

        let coingecko = CoinGeckoProvider::get();
        let price = coingecko.get_price_by_id("solana", "usd").await.unwrap();

        tracing::info!("price: {}", price);
    }
}
