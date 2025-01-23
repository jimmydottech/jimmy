use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::Action;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TwitterAction {
    InvestorMemo {
        tweet_id: String,
        tweet_text: String,
    },
}

impl ToString for TwitterAction {
    fn to_string(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize TwitterAction")
    }
}

impl FromStr for TwitterAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(Into::into)
    }
}

impl Action for TwitterAction {
    fn prompt(&self) -> String {
        match self {
            TwitterAction::InvestorMemo {
                tweet_id,
                tweet_text,
            } => {
                format!(
                    "Post Investor Memo from tweet {} with the following text: {}",
                    tweet_id, tweet_text
                )
            }
        }
    }
}
