use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::Action;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedAction {
    Substack { url: String, text: String },
}

impl ToString for FeedAction {
    fn to_string(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize FeedAction")
    }
}

impl FromStr for FeedAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(Into::into)
    }
}

impl Action for FeedAction {
    fn prompt(&self) -> String {
        match self {
            FeedAction::Substack { url, text } => {
                format!(
                    "Read Substack from {} with the following text: {}",
                    url, text
                )
            }
        }
    }
}
