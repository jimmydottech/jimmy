use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{config::Config, token::store::SolanaTokenStore};

use super::{Feed, FeedType};

pub struct SubstackFeed {
    urls: Vec<String>,
    latest: Arc<Mutex<HashMap<String, String>>>,
}

impl SubstackFeed {
    pub fn new() -> Self {
        let subs = Config::get().substack_urls.clone();
        Self {
            urls: subs,
            latest: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[allow(dead_code)]
    pub fn from_urls(urls: &[impl AsRef<str>]) -> Self {
        Self {
            urls: urls.iter().map(|url| url.as_ref().to_string()).collect(),
            latest: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Feed for SubstackFeed {
    fn feed_type(&self) -> super::FeedType {
        FeedType::Newsletter
    }

    fn construct_prompt(&self, content: String) -> String {
        let tokens = SolanaTokenStore::get().tokens();
        let tokens_str = tokens
            .iter()
            .map(|(_, v)| v.symbol.clone())
            .collect::<Vec<String>>();

        let prompt = format!(
            r#"Please read the following news article {content}. Then, from this list of valid tokens {tokens:?},
            only select the tokens that reflect noteworthy or investable opportunities based on the article’s content.
            Then output a JSON array containing only these cryptocurrency tickers, with no additional text
        or explanation—an example would be ["BTC", "ETH", "SOL"]. Please adhere to the following requirements:
        1. Only output an array of cryptocurrency tickers, such as ["BTC", "ETH", "SOL"]."
        2. Do not include any additional text or explanation.
        3. The order and number of items in the array should reflect the information provided in the article and must be consistent with its content.
        4. You must only pick from the valid token list provided (i.e., do not invent or include tokens not on the list)
        Remove any near-duplicate tokens (include them only if the article explicitly mentions their unique use or relevance)."#,
            content = content,
            tokens = tokens_str
        );

        prompt
    }

    async fn fetch(&self) -> anyhow::Result<Option<String>> {
        let mut contents = String::new();
        let mut latest = self.latest.lock().await;
        for url in &self.urls {
            let rss = reqwest::get(url).await?.bytes().await?;
            let channel = rss::Channel::read_from(&rss[..])?;

            for item in channel.items().iter().take(5) {
                let title = item.title().unwrap_or("No Title");

                if let Some(prev_title) = latest.get(title) {
                    if prev_title == title {
                        break;
                    }
                }

                latest.insert(title.to_string(), title.to_string());

                let content = item.content().unwrap_or("No Content");

                contents.push_str(&format!("{title} \n {content} \n\n"));
            }
        }

        if contents.is_empty() {
            return Ok(None);
        }

        Ok(Some(contents))
    }
}

#[tokio::test]
async fn test_get_rss_feed() {
    let url = "https://www.thetokendispatch.com/feed";
    let content = reqwest::get(url).await.unwrap().bytes().await.unwrap();

    let channel = rss::Channel::read_from(&content[..]).unwrap();
    println!("Feed Title: {}", channel.title());

    for item in channel.items().iter().take(5) {
        println!("Title: {}", item.title().unwrap_or("No Title"));
        println!("Link: {}", item.link().unwrap_or("No Link"));
        println!("Content: {}", item.content().unwrap_or("No Content"));
        println!(
            "Description: {}",
            item.description().unwrap_or("No Description")
        );
        println!("-------------------------------");
    }
}

#[tokio::test]
async fn test_substack_feed() {
    use crate::llm::azure::run_prompt;
    dotenv::dotenv().ok();
    let feed = SubstackFeed::from_urls(&["https://www.thetokendispatch.com/feed"]);
    let content = feed.fetch().await.unwrap().unwrap();
    println!("{}", content);

    let prompt = feed.construct_prompt(content);
    let resp = run_prompt(prompt).await.unwrap();

    let list = serde_json::from_str::<Vec<String>>(&resp).unwrap();
    println!("{:#?}", list);
}
