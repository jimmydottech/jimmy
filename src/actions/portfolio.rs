use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::{Action, ActionLog};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PortfolioAction {
    Buy {
        token: String,
        amount: u64,
        sol: u64,
        tx_sig: String,
    },
    Sell {
        token: String,
        amount: u64,
        sol: u64,
        tx_sig: String,
    },
    Pnl {
        token: String,
        pnl: f64,
        sell_action: ActionLog,
    },
}

impl PortfolioAction {
    pub fn buy(token: String, amount: u64, sol: u64, tx_sig: String) -> Self {
        PortfolioAction::Buy {
            token,
            amount,
            sol,
            tx_sig,
        }
    }

    pub fn sell(token: String, amount: u64, sol: u64, tx_sig: String) -> Self {
        PortfolioAction::Sell {
            token,
            amount,
            sol,
            tx_sig,
        }
    }

    pub fn pnl(token: String, pnl: f64, sell_action: ActionLog) -> Self {
        PortfolioAction::Pnl {
            token,
            pnl,
            sell_action,
        }
    }
}

impl ToString for PortfolioAction {
    fn to_string(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize PortfolioAction")
    }
}

impl FromStr for PortfolioAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(Into::into)
    }
}

impl Action for PortfolioAction {
    fn prompt(&self) -> String {
        match self {
            PortfolioAction::Buy {
                token,
                amount,
                sol,
                tx_sig,
            } => {
                format!("Buy {amount} tokens of {token} with {sol} SOL(LAMPORT) which tx signature is {tx_sig}")
            }
            PortfolioAction::Sell {
                token,
                amount,
                sol,
                tx_sig,
            } => {
                format!("Sell {amount} tokens of {token} with {sol} SOL(LAMPORT) which tx signature is {tx_sig}")
            }
            PortfolioAction::Pnl {
                token,
                pnl,
                sell_action,
            } => {
                let action = PortfolioAction::from_log(sell_action)
                    .expect("must be sell action")
                    .prompt();
                format!("Realize PnL of {pnl} SOL(LAMPORT) from token {token} from sell action \"{action}\"", )
            }
        }
    }
}
