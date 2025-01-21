use anyhow::Result;
use serde_json::Value;

pub async fn get_price(tokens: Vec<String>) -> Result<Vec<f64>> {
    // // Jupiter API endpoint
    // let api_url = "https://api.jup.ag/price/v2";

    // // Tokens to fetch prices for (using mint addresses or symbols)
    // let ids = tokens.join(",");
    // let show_extra_info = false;
    // // vsToken cannot be used with showExtraInfo when it's true. A response 400: Bad request would be returned.
    // // If we don't specify vsToken, the price will be in USDC.
    // let vs_token = "SOL";

    // let url = format!("{api_url}?ids={ids}&showExtraInfo={show_extra_info}");

    // let response = reqwest::get(&url).await?;
    // let json: Value = response.json().await?;

    // tracing::info!("Jupiter price response: {}", json);

    todo!()
}
