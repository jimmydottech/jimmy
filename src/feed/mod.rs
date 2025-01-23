mod newsletter;
pub mod substack;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait Feed {
    async fn fetch(&self) -> Result<Option<String>>;
    fn construct_prompt(&self, content: String) -> String;
    fn feed_type(&self) -> FeedType;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedType {
    Newsletter,
}
