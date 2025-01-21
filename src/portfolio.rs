use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use tokio::sync::OnceCell;
use tracing::instrument;

use crate::{
    actions::{portfolio::PortfolioAction, Action},
    jupiter::swap::{swap_from_sol, swap_to_sol},
    store::{map::StoreMap, LocalStore, Store},
    token::{jimmy::JimmyToken, structs::TokenInfo},
    wallet::Wallet,
    LAMPORTS_PER_SOL,
};

pub struct Portfolio {
    jimmy_token: JimmyHolding,
    tokens: StoreMap<String, OtherTokenHolding, LocalStore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtherTokenHolding {
    pub token_info: TokenInfo,
    pub shots: Vec<OneShot>,
    /// Total profit and loss
    pub total_pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneShot {
    pub quantity: u64,
    pub total_cost_in_sol: f64,
}

impl OneShot {
    pub fn avg_cost(&self) -> f64 {
        self.total_cost_in_sol / self.quantity as f64
    }
}

impl OtherTokenHolding {
    pub fn init(token_info: TokenInfo) -> Self {
        Self {
            token_info,
            shots: vec![],
            total_pnl: 0.0,
        }
    }

    pub fn holding_amount(&self) -> u64 {
        self.shots.iter().map(|shot| shot.quantity).sum()
    }

    pub fn holding_ui_amount(&self) -> f64 {
        self.decimal(self.holding_amount())
    }

    pub fn total_cost(&self) -> f64 {
        self.shots.iter().map(|shot| shot.total_cost_in_sol).sum()
    }

    pub fn decimal(&self, amount: u64) -> f64 {
        amount as f64 / 10u64.pow(self.token_info.decimals as u32) as f64
    }

    #[instrument(name = "Holding::UpdateBuy", skip(self))]
    pub fn update_buy(&mut self, sol_spend: u64, token_increase: u64) {
        tracing::debug!("update buy");

        let shot = OneShot {
            quantity: token_increase,
            total_cost_in_sol: sol_spend as f64,
        };

        self.shots.push(shot);
    }

    #[instrument(name = "Holding::UpdateSell", skip(self))]
    pub fn update_sell(&mut self, sol_earned: u64, token_decrease: u64) {
        tracing::debug!("update sell");

        let mut remaining = token_decrease;
        let mut cost = 0.0;

        while let Some(shot) = self.shots.pop() {
            let cut = remaining.min(shot.quantity);
            remaining -= cut;
            cost += shot.avg_cost() * cut as f64;

            if remaining == 0 {
                if cut < shot.quantity {
                    let total_cost_in_sol = shot.avg_cost() * (shot.quantity - cut) as f64;
                    let new_shot = OneShot {
                        quantity: shot.quantity - cut,
                        total_cost_in_sol,
                    };
                    self.shots.push(new_shot);
                }
                break;
            }
        }

        assert_eq!(remaining, 0);

        self.total_pnl += sol_earned as f64 - cost;
    }

    pub fn profit_margin(&self, current_price_in_sol: f64) -> f64 {
        let current_value =
            self.holding_ui_amount() * current_price_in_sol * LAMPORTS_PER_SOL as f64;
        let profit = current_value - self.total_cost();

        if self.total_cost() == 0.0 {
            return 0.0;
        }

        profit / self.total_cost()
    }

    pub fn profit_margin_from_usd(&self, sol_in_usd: f64, token_in_usd: f64) -> f64 {
        self.profit_margin(token_in_usd / sol_in_usd)
    }
}

impl Portfolio {
    const HOLDING_TOKENS_PREFIX: &'static str = "holding_tokens";

    pub async fn get() -> &'static Portfolio {
        static PORTFOLIO: OnceCell<Portfolio> = OnceCell::const_new();
        PORTFOLIO
            .get_or_init(|| async {
                Portfolio::init()
                    .await
                    .expect("Failed to initialize portfolio")
            })
            .await
    }

    async fn init() -> anyhow::Result<Self> {
        let jimmy_token = JimmyToken::get().await;

        let jimmy_holding = JimmyHolding {
            mint: jimmy_token.mint_pubkey(),
        };

        Ok(Self {
            jimmy_token: jimmy_holding,
            tokens: LocalStore::open_map(Self::HOLDING_TOKENS_PREFIX),
        })
    }

    pub async fn jimmy_balance(&self) -> anyhow::Result<u64> {
        let jimmy_mint = self.jimmy_token.mint;
        let wallet = Wallet::get();
        let balance = wallet.get_token_balance(&jimmy_mint).await?;

        Ok(balance.amount.parse::<u64>()?)
    }

    pub async fn sol_balance(&self) -> anyhow::Result<u64> {
        let wallet = Wallet::get();
        let balance = wallet.balance()?;

        Ok(balance)
    }

    pub fn tokens(&self) -> &StoreMap<String, OtherTokenHolding, LocalStore> {
        &self.tokens
    }

    pub async fn sell_token(
        &self,
        token_info: &TokenInfo,
        token_amount: u64,
    ) -> anyhow::Result<()> {
        let mut token_holding = self
            .tokens()
            .get(&token_info.symbol)?
            .ok_or(anyhow::anyhow!(
                "Token Holding {} not found",
                token_info.symbol
            ))?;
        let old_pnl = token_holding.total_pnl;
        if token_holding.holding_amount() < token_amount {
            return Err(anyhow::anyhow!("Not enough tokens to sell"));
        }

        let (sol_amount, sig) = swap_to_sol(&token_info.address.to_string(), token_amount).await?;
        token_holding.update_sell(sol_amount, token_amount);
        let this_pnl = token_holding.total_pnl - old_pnl;

        // action log
        {
            let sell_action = PortfolioAction::sell(
                token_info.address.to_string(),
                token_amount,
                sol_amount,
                sig.to_string(),
            );
            let pnl_action = PortfolioAction::pnl(
                token_info.address.to_string(),
                this_pnl,
                sell_action.into_log(),
            );
            sell_action.log();
            pnl_action.log();
        }

        self.tokens()
            .insert(token_info.symbol.clone(), token_holding)?;
        Ok(())
    }

    pub async fn buy_token(&self, token_info: &TokenInfo, sol_amount: u64) -> anyhow::Result<()> {
        let sol_balance = self.sol_balance().await?;
        if sol_balance < sol_amount {
            return Err(anyhow::anyhow!("Not enough SOL to buy"));
        }

        let mut token_holding = match self.tokens().get(&token_info.symbol)? {
            Some(holding) => holding,
            None => {
                let holding = OtherTokenHolding::init(token_info.clone());
                self.tokens()
                    .insert(token_info.symbol.clone(), holding.clone())?;
                holding
            }
        };

        let (token_amount, tx_sig) =
            swap_from_sol(&token_info.address.to_string(), sol_amount).await?;
        token_holding.update_buy(sol_amount, token_amount);

        // action log
        PortfolioAction::buy(
            token_info.address.to_string(),
            token_amount,
            sol_amount,
            tx_sig.to_string(),
        )
        .log();

        self.tokens()
            .insert(token_info.symbol.clone(), token_holding)?;
        Ok(())
    }

    pub async fn buy_jimmy(&self, sol_amount: u64) -> anyhow::Result<()> {
        let sol_balance = self.sol_balance().await?;
        if sol_balance < sol_amount {
            return Err(anyhow::anyhow!("Not enough SOL to buy"));
        }

        let (jimmy_amount, sig) =
            swap_from_sol(&self.jimmy_token.mint.to_string(), sol_amount).await?;

        // action log
        PortfolioAction::buy(
            self.jimmy_token.mint.to_string(),
            jimmy_amount,
            sol_amount,
            sig.to_string(),
        )
        .log();

        Ok(())
    }

    pub async fn sell_jimmy(&self, jimmy_amount: u64) -> anyhow::Result<()> {
        let jimmy_balance = self.jimmy_balance().await?;
        if jimmy_balance < jimmy_amount {
            return Err(anyhow::anyhow!("Not enough JIMMY to sell"));
        }

        let (sol_amount, sig) =
            swap_to_sol(&self.jimmy_token.mint.to_string(), jimmy_amount).await?;

        // action log
        PortfolioAction::sell(
            self.jimmy_token.mint.to_string(),
            jimmy_amount,
            sol_amount,
            sig.to_string(),
        )
        .log();

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct JimmyHolding {
    pub mint: Pubkey,
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use crate::{
        portfolio::{OtherTokenHolding, Portfolio},
        token::{store::SolanaTokenStore, structs::TokenInfo},
        LAMPORTS_PER_SOL,
    };

    pub fn init() {
        dotenv::dotenv().unwrap();
        std::env::set_var(
            "SOLANA_RPC_URL",
            "https://mainnet.helius-rpc.com/?api-key=f180fde7-daa5-45c8-9356-e216dc9ff367",
        );
        std::env::set_var("RUST_LOG", "debug");
        tracing_subscriber::fmt::init();
        // Wallet::get().get_airdrop(5 * LAMPORTS_PER_SOL).await.unwrap();
    }

    #[test]
    fn test_calculate_profit() {
        use solana_sdk::pubkey::Pubkey;

        let mut holding = OtherTokenHolding {
            token_info: TokenInfo {
                address: Pubkey::default(),
                symbol: "TEST".to_string(),
                name: "Test".to_string(),
                decimals: 6,
                coingecko_id: None,
            },
            shots: vec![],
            total_pnl: 0.0,
        };

        holding.update_buy(10_i32.pow(9) as u64, 10_i32.pow(6) as u64);
        holding.update_sell(10_i32.pow(9) as u64, 5 * 10_i32.pow(5) as u64);

        let profit = holding.profit_margin(1.0);
        println!("Profit: {}", profit);
        assert_eq!(profit, 0.5);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_more_profit() {
        init();
        let store = SolanaTokenStore::get();
        let token_info = store.get_token_info("USDC").await.unwrap().unwrap();

        let mut has_sol = 200 * LAMPORTS_PER_SOL;

        pub fn ui_sol(sol: u64) -> f64 {
            sol as f64 / 10_i32.pow(9) as f64
        }

        let mut holding = OtherTokenHolding {
            token_info: token_info.clone(),
            shots: vec![],
            total_pnl: 0.0,
        };

        let usdc_dec = 10_i32.pow(token_info.decimals.clone() as u32) as f64;
        let mut rng = rand::thread_rng();
        // sol_price = 1 sol per usdc
        // 1 usdc per sol = 1 / sol_price
        for i in 0..5 {
            let sol_price = rng.gen_range(6.8..7.3);
            tracing::info!(
                "When Buy Token Price: {}, Sol price {}",
                1.0 / sol_price,
                sol_price
            );
            let token_increase = ((ui_sol(has_sol) * sol_price) * usdc_dec) as u64;
            tracing::info!("token increase: {}", token_increase);
            holding.update_buy(has_sol, token_increase);
            let sol_price = rng.gen_range(6.8..7.3);
            tracing::info!("When Sell Token Price: {}", 1.0 / sol_price);
            let sell_part = rng.gen_range(0.5..0.9);
            let token_decrease = (holding.holding_amount() as f64 * sell_part) as u64;
            tracing::info!("token decrease: {}", token_decrease);
            has_sol = ((holding.holding_ui_amount() * sell_part / sol_price as f64)
                * LAMPORTS_PER_SOL as f64) as u64;
            tracing::info!("earn sol: {}", ui_sol(has_sol));
            holding.update_sell(
                has_sol,
                (holding.holding_amount() as f64 * sell_part) as u64,
            );
            tracing::info!("holding: {:?}", holding);
            tracing::info!(
                "remain token to sol {}",
                holding.holding_ui_amount() / sol_price
            );
            tracing::info!(
                "Profit: {}, Round {}, holding amount {}",
                holding.profit_margin(1.0 / sol_price),
                i,
                holding.holding_ui_amount()
            );
        }

        tracing::info!("final holding: {:?}", holding);
        let sol_price = rng.gen_range(6.8..7.3);
        tracing::info!("sol price: {}", sol_price);
        tracing::info!("Final SOL {}", holding.holding_ui_amount() / sol_price);
        tracing::info!("Final Profit: {}", holding.profit_margin(1.0 / sol_price));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_buy_and_sell_token() {
        init();
        let portfolio = Portfolio::init().await.unwrap();
        let token_store = SolanaTokenStore::get();
        let usdc = token_store.get_token_info("USDC").await.unwrap().unwrap();
        tracing::info!("USDC: {:?}", usdc);

        let sol_balance = portfolio.sol_balance().await.unwrap();
        tracing::info!("SOL Balance: {}", sol_balance);

        portfolio.buy_token(&usdc, sol_balance / 10).await.unwrap();
        tracing::info!(
            "sol balance after buying: {}",
            portfolio.sol_balance().await.unwrap()
        );
        let usdc_balance = portfolio
            .tokens()
            .get(&"USDC".to_string())
            .unwrap()
            .unwrap()
            .holding_amount();
        tracing::info!("usdc balance after buying: {}", usdc_balance);

        portfolio.sell_token(&usdc, usdc_balance).await.unwrap();

        tracing::info!(
            "sol balance after selling: {}",
            portfolio.sol_balance().await.unwrap()
        );

        let usdc_balance = portfolio
            .tokens()
            .get(&"USDC".to_string())
            .unwrap()
            .unwrap()
            .holding_amount();

        tracing::info!("usdc balance after selling: {}", usdc_balance);
    }
}
