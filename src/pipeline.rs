use anyhow::Result;

use std::collections::HashSet;
use std::time::Duration;

use crate::actions::portfolio::PortfolioAction;
use crate::actions::twitter::TwitterAction;
use crate::actions::utils::get_cur_timestamp;
use crate::actions::Action;
use crate::config::Config;
use crate::constant::*;
use crate::feed::{Feed, FeedType};
use crate::llm::azure::run_prompt;
use crate::llm::scorer::score_reply;
use crate::portfolio::Portfolio;
use crate::price::coingecko::CoinGeckoProvider;
use crate::strategy::select_tokens;
use crate::token::jimmy::JimmyToken;
use crate::twitter::{Reply, TweetType, TwitterClient, TwitterPrompt};

pub struct Pipeline {
    feeds: Vec<Box<dyn Feed>>,
}

impl Pipeline {
    fn new(feeds: Vec<Box<dyn Feed>>) -> Self {
        Self { feeds }
    }

    pub async fn run_once(&self, sell_jimmy: bool) -> Result<()> {
        tracing::info!("Running pipeline once");

        let mut candidates: HashSet<String> = HashSet::new();
        for feed in self.feeds.iter() {
            match feed.feed_type() {
                FeedType::Newsletter => {
                    if let Some(content) = feed.fetch().await? {
                        let prompt = feed.construct_prompt(content);
                        let response = run_prompt(prompt).await?;
                        let tokens: Vec<String> = serde_json::from_str::<Vec<String>>(&response)?
                            .iter()
                            .map(|token| token.trim().trim_start_matches('$').to_string())
                            .collect();
                        tracing::info!("Recommended tokens in newsletter: {}", tokens.join(", "));

                        candidates.extend(tokens);
                    }
                }
            }
        }

        let trades = select_tokens(candidates).await?;

        let portfolio = Portfolio::get().await;

        let config = Config::get();
        if sell_jimmy {
            let amount = config.sell_jimmy_amount * JimmyToken::one_jimmy() as f64;
            portfolio.sell_jimmy(amount as u64).await?;
        }

        // Sell tokens that are gaining profit and not in the trades
        for (_, token_holding) in portfolio.tokens().iter() {
            let token_holding = token_holding.into_owned();

            let in_trades = trades
                .iter()
                .find(|t| t.token == token_holding.token_info)
                .is_some();
            if in_trades {
                continue;
            }

            let profit = {
                let price_provider = CoinGeckoProvider::get();
                let prices =
                    if let Some(coingecko_id) = token_holding.token_info.coingecko_id.as_ref() {
                        price_provider
                            .get_prices_by_ids(&[&coingecko_id, SOL_COINGECKO_ID], USD_CURRENCY)
                            .await?
                    } else {
                        return Err(anyhow::anyhow!("No coingecko id found"));
                    };

                let token_price = prices[0];
                let sol_price = prices[1];
                token_holding.profit_margin_from_usd(sol_price, token_price)
            };

            if profit > config.min_profit_rate {
                let amount = token_holding.holding_amount();
                portfolio
                    .sell_token(&token_holding.token_info, amount)
                    .await?;
            }
        }

        // Sell Jimmy to get money
        let amount_to_buy = LAMPORTS_PER_SOL as f64 * config.max_sol_trading_amount_one_day;
        while (portfolio.sol_balance().await? as f64) < amount_to_buy {
            let sell_jimmy_amount = config.sell_jimmy_amount * JimmyToken::one_jimmy() as f64;
            portfolio.sell_jimmy(sell_jimmy_amount as u64).await?;
        }

        // Buy tokens
        for trade in trades {
            let sol_amount = (amount_to_buy * trade.weight).floor() as u64;
            if let Err(e) = portfolio.buy_token(&trade.token, sol_amount).await {
                tracing::error!("Failed to buy token: {}", e);
            }
        }

        // handle investor memo
        self.handle_investor_memo().await?;

        Ok(())
    }

    pub async fn handle_investor_memo(&self) -> anyhow::Result<()> {
        const ACTIVE_TIME: u64 = 60 * 60 * 24; // 1 day
        let last_time = match TwitterAction::iter().next() {
            Some((_, raw)) => {
                if raw.timestamp() + ACTIVE_TIME > get_cur_timestamp() {
                    return Ok(());
                }
                raw.timestamp()
            }
            None => get_cur_timestamp() - ACTIVE_TIME,
        };

        // Get Portfolio Action from the last time
        let mut prompts = vec![];
        for (action, raw) in PortfolioAction::iter() {
            if raw.timestamp() > last_time {
                prompts.push(action.prompt());
            }
        }

        let tweet_prompt =
            TwitterPrompt::new(TweetType::InvestorMemo, prompts.clone(), None)?.build();
        let mut response = run_prompt(tweet_prompt).await?;
        while response.len() > 275 {
            tracing::warn!("Tweet text is too long, retrying...");
            tokio::time::sleep(Duration::from_secs(5)).await;
            let tweet_prompt =
                TwitterPrompt::new(TweetType::InvestorMemo, prompts.clone(), None)?.build();
            response = run_prompt(tweet_prompt).await?;
        }
        tracing::info!("Tweet text: {}", response);

        let twitter_client = TwitterClient::get();
        let tweet_id = twitter_client.post_tweet(&response).await?;

        // log the action
        TwitterAction::InvestorMemo {
            tweet_id,
            tweet_text: response,
        }
        .log();

        Ok(())
    }

    pub async fn handle_twitter_replies(&self) -> Result<()> {
        tracing::info!("Handling Twitter replies");

        let twitter_client = TwitterClient::get();
        let replies = match twitter_client.get_replies().await {
            Ok(replies) => {
                tracing::info!("New replies: {:?}", replies);
                replies
            }
            Err(e) => {
                tracing::error!("Failed to get Twitter replies: {}", e);
                return Ok(());
            }
        };

        let mut scored_replies: Vec<(Reply, u8)> = Vec::new();
        for reply in replies {
            let score = score_reply(&reply.text)
                .await
                .inspect_err(|e| {
                    tracing::error!("Failed to score reply: {}", e);
                })
                .unwrap_or(0);
            scored_replies.push((reply, score));
        }
        scored_replies.sort_by(|a, b| b.1.cmp(&a.1));

        let top_replies = scored_replies
            .iter()
            .filter(|(_, score)| *score > 2)
            .take(3)
            .collect::<Vec<_>>();

        for (reply, score) in top_replies {
            tracing::info!(
                "Jimmy decides to reply to: {} (score: {})",
                reply.text,
                score
            );

            let conversations = reply.conversations.join("\n");
            let tweet_prompt =
                TwitterPrompt::new(TweetType::Engagement, vec![], Some(conversations.clone()))?
                    .build();
            let mut response = run_prompt(tweet_prompt).await?;
            while response.len() > 275 {
                tracing::warn!("Tweet text is too long, retrying...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                let tweet_prompt =
                    TwitterPrompt::new(TweetType::Engagement, vec![], Some(conversations.clone()))?
                        .build();
                response = run_prompt(tweet_prompt).await?;
            }
            tracing::info!("Tweet reply: {}", response);

            twitter_client.reply_to(&reply.id, &response).await?;

            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        Ok(())
    }

    pub async fn run_loop(&self) -> Result<()> {
        let mut trading_round = 1;
        let trading_interval = Duration::from_secs(60 * 60 * 24);
        let mut trading_timer = tokio::time::interval(trading_interval);

        let twitter_interval = Duration::from_secs(60 * 5);
        let mut twitter_timer = tokio::time::interval(twitter_interval);

        loop {
            tokio::select! {
                _ = trading_timer.tick() => {
                    tracing::info!("Running trading round {}", trading_round);

                    if let Err(e) = self.run_once(false).await {
                        tracing::error!("Failed to run trading pipeline: {}", e);
                    }

                    tracing::info!("Trading round {} completed", trading_round);
                    trading_round += 1;
                }
                _ = twitter_timer.tick() => {
                    if let Err(e) = self.handle_twitter_replies().await {
                        tracing::error!("Failed to handle Twitter replies: {}", e);
                    }
                }
            }
        }
    }
}

pub struct PipelineBuilder {
    feeds: Vec<Box<dyn Feed>>,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        Self { feeds: vec![] }
    }

    pub fn build(self) -> Pipeline {
        Pipeline::new(self.feeds)
    }

    pub fn with_feed(mut self, feed: impl Feed + 'static) -> Self {
        self.feeds.push(Box::new(feed));
        self
    }
}
