use anyhow::Result;

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::constant::*;
use crate::price::coingecko::CoinGeckoProvider;
use crate::token::store::SolanaTokenStore;
use crate::token::structs::TokenInfo;

#[derive(Debug)]
pub struct Trade {
    pub token: TokenInfo,
    // weight of the token in the portfolio
    // from 0 to 1
    pub weight: f64,
}

impl std::fmt::Display for Trade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Trade: {}, weight: {}", self.token, self.weight)
    }
}

#[derive(Debug)]
pub struct CandidatePerformance {
    pub token: TokenInfo,
    pub hold_profit_rate: f64,
    pub max_profit_rate: f64,
}

impl CandidatePerformance {
    pub fn profit_rate(&self) -> f64 {
        self.hold_profit_rate
    }
}

/// Compares two CandidatePerformance instances.
///
/// This function compares primarily by max_profit_rate in descending order,
/// and in case of a tie, by hold_profit_rate in descending order.
fn compare_candidate_performance(a: &CandidatePerformance, b: &CandidatePerformance) -> Ordering {
    b.profit_rate()
        .partial_cmp(&a.profit_rate())
        .unwrap_or(Ordering::Equal)
}

pub async fn select_tokens(candidates: HashSet<String>) -> Result<Vec<Trade>> {
    tracing::info!("Start selecting tokens...");

    let price_provider = CoinGeckoProvider::get();

    let mut candidate_tokens = Vec::new();
    let token_store = SolanaTokenStore::get();
    for candidate in candidates {
        if let Some(token_info) = token_store.get_token_info(&candidate).await? {
            if token_info.coingecko_id.is_some() {
                candidate_tokens.push(token_info);
            } else {
                tracing::error!("Not found coingecko id for {}, ignore it", candidate);
            }
        } else {
            tracing::error!("Token not found: {}, ignore it", candidate);
            continue;
        }
    }

    let mut candidate_performances = Vec::new();
    for token_info in candidate_tokens {
        let coin_id = token_info.coingecko_id.as_ref().unwrap();
        let historical_price = if let Ok(price) = price_provider
            .get_historical_price_by_id(&coin_id, USD_CURRENCY, 3)
            .await
        {
            price
        } else {
            tracing::error!("Failed to get historical price for {}", coin_id);
            continue;
        };

        let hold_profit_rate = calculate_hold_profit_rate(&historical_price.prices).await?;
        let max_profit_rate = calculate_max_profit_rate(&historical_price.prices).await?;

        tracing::info!(
            "{:<10}: hold_profit_rate: {:>6.2}%, max_profit_rate: {:>6.2}%",
            token_info.symbol,
            hold_profit_rate * 100.0, // Convert to percentage
            max_profit_rate * 100.0   // Convert to percentage
        );

        let candidate_performance = CandidatePerformance {
            token: token_info,
            hold_profit_rate,
            max_profit_rate,
        };

        candidate_performances.push(candidate_performance);
    }

    candidate_performances.sort_by(compare_candidate_performance);

    let top_performances: Vec<&CandidatePerformance> =
        candidate_performances.iter().take(3).collect();
    let top_symbols = top_performances
        .iter()
        .map(|p| p.token.symbol.clone())
        .collect::<Vec<String>>()
        .join(", ");
    tracing::info!("Top performances: {}", top_symbols);

    let weights = vec![5, 3, 2];
    let total_parts: i32 = weights.iter().sum(); // Total parts = 10

    let mut trades = vec![];
    for (i, performance) in top_performances.iter().enumerate() {
        let weight = weights[i] as f64 / total_parts as f64;
        trades.push(Trade {
            token: performance.token.clone(),
            weight,
        });
    }

    for trade in &trades {
        tracing::info!("{}", trade);
    }

    Ok(trades)
}

/// Calculates the profit rate from holding a token over a specified period.
///
/// This function simulates buying the token at the beginning of the period
/// and selling it at the end. The profit rate is calculated as a percentage
/// based on the difference between the selling price and the buying price.
///
/// # Arguments
///
/// * `prices`: A 2D vector where each sub-vector contains:
///   - `timestamp`: The time at which the price was recorded (in UNIX timestamp).
///   - `price`: The price of the token at that timestamp.
///
/// # Returns
///
/// Returns the profit rate as a floating-point number representing the
/// percentage change in price. The calculation is done using the formula:
///
///     Profit Rate = (Sell Price - Buy Price) / Buy Price
///
/// If the input is empty, the function returns 0.0.
async fn calculate_hold_profit_rate(prices: &Vec<Vec<f64>>) -> Result<f64> {
    if prices.is_empty() {
        return Ok(0.0);
    }

    let buy_price = prices[0][1];
    let sell_price = prices[prices.len() - 1][1];

    Ok((sell_price - buy_price) / buy_price)
}

/// Calculates the maximum profit rate that can be achieved from holding a token
/// over a specified period based on historical price data.
///
/// This function simulates buying the token at the lowest price within the
/// given period and selling it at the highest price. The goal is to determine
/// the best possible profit that could have been made by strategically
/// choosing the buy and sell points.
///
/// # Arguments
///
/// * `prices`: A 2D vector where each sub-vector contains:
///   - `timestamp`: The time at which the price was recorded (in UNIX timestamp).
///   - `price`: The price of the token at that timestamp.
///
/// # Returns
///
/// Returns the maximum profit rate as a floating-point number representing the
/// percentage change in price. The calculation is done using the formula:
///
///     Profit Rate = ((Sell Price - Buy Price) / Buy Price) * 100
///
/// If no profit can be made (e.g., if prices are always decreasing), the function will return 0.0.
async fn calculate_max_profit_rate(prices: &Vec<Vec<f64>>) -> Result<f64> {
    if prices.is_empty() {
        return Ok(0.0);
    }

    let mut min_price = f64::MAX;
    let mut max_profit_rate = 0.0;

    for price in prices.iter().map(|p| p[1]) {
        if price < min_price {
            min_price = price;
        } else {
            let profit_rate = (price - min_price) / min_price;
            if profit_rate > max_profit_rate {
                max_profit_rate = profit_rate;
            }
        }
    }

    Ok(max_profit_rate)
}
