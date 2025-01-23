use anyhow::Result;
use serde::{Deserialize, Serialize};
use tweety_rs::api::mentions::{
    ExpansionType, QueryParams as MentionsQueryParams, ReferencedTweet, TweetField, UserField,
};
use tweety_rs::api::tweet::QueryParams as TweetQueryParams;
use tweety_rs::types::tweet::{PostTweetParams, Reply as ReplyParams};
use tweety_rs::TweetyClient;

use std::sync::OnceLock;

use crate::config::Config;
use crate::store::{LocalStore, Store, StoreMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TweetInfo {
    pub id: String,
    pub text: String,
    pub author_id: String,
    pub conversation_id: String,
    pub in_reply_to_user_id: Option<String>,
    pub referenced_tweets: Option<Vec<ReferencedTweet>>,
}

#[derive(Debug)]
pub struct Reply {
    pub id: String,
    pub text: String,
    pub author_id: String,
    pub conversation_id: String,
    pub in_reply_to_user_id: String,
    pub referenced_tweets: Vec<ReferencedTweet>,
    pub conversations: Vec<String>,
}

pub struct TwitterClient {
    use_twitter: bool,
    client: TweetyClient,
    my_tweets: StoreMap<String, String, LocalStore>,
    my_replies: StoreMap<String, String, LocalStore>,
    fetched_tweets: StoreMap<String, TweetInfo, LocalStore>,
    fetched_users: StoreMap<String, UserInfo, LocalStore>,
}

impl TwitterClient {
    pub fn get() -> &'static TwitterClient {
        static INSTANCE: OnceLock<TwitterClient> = OnceLock::new();
        INSTANCE.get_or_init(|| Self::new())
    }

    fn new() -> Self {
        let config = Config::get();
        let client = TweetyClient::new(
            &config.twitter_consumer_key,
            &config.twitter_access_token,
            &config.twitter_consumer_key_secret,
            &config.twitter_access_token_secret,
        );
        let use_twitter = config.use_twitter;

        Self {
            client,
            use_twitter,
            my_tweets: LocalStore::open_map("my_tweets"),
            my_replies: LocalStore::open_map("my_replies"),
            fetched_tweets: LocalStore::open_map("fetched_tweets"),
            fetched_users: LocalStore::open_map("fetched_users"),
        }
    }

    pub async fn profile_url(&self) -> Result<String> {
        let user_info = self.user_info().await?;
        Ok(format!("https://x.com/{}", user_info.username))
    }

    pub async fn user_info(&self) -> Result<UserInfo> {
        if !self.use_twitter {
            return Ok(UserInfo {
                id: "".to_string(),
                name: "".to_string(),
                username: "".to_string(),
            });
        }

        static USER_INFO: OnceLock<UserInfo> = OnceLock::new();
        if let Some(user_info) = USER_INFO.get() {
            return Ok(user_info.clone());
        }

        let res = self.client.get_user_me(None).await?;
        tracing::info!("User me: {:?}", res);
        let id = res
            .get("data")
            .ok_or(anyhow::anyhow!("No data found"))?
            .get("id")
            .ok_or(anyhow::anyhow!("No id found"))?
            .as_str()
            .ok_or(anyhow::anyhow!("Failed to parse id"))?;
        let name = res
            .get("data")
            .ok_or(anyhow::anyhow!("No data found"))?
            .get("name")
            .ok_or(anyhow::anyhow!("No name found"))?
            .as_str()
            .ok_or(anyhow::anyhow!("Failed to parse name"))?;
        let username = res
            .get("data")
            .ok_or(anyhow::anyhow!("No data found"))?
            .get("username")
            .ok_or(anyhow::anyhow!("No username found"))?
            .as_str()
            .ok_or(anyhow::anyhow!("Failed to parse username"))?;

        let user_info = UserInfo {
            id: id.to_string(),
            name: name.to_string(),
            username: username.to_string(),
        };
        let _ = USER_INFO.set(user_info.clone());

        self.fetched_users
            .insert(user_info.id.clone(), user_info.clone())?;

        Ok(user_info)
    }

    pub async fn post_tweet(&self, tweet: &str) -> Result<String> {
        if self.use_twitter {
            tracing::info!("Tweet posted: \n{}", tweet);

            let body_param = PostTweetParams {
                direct_message_deep_link: None,
                for_super_followers_only: None,
                geo: None,
                media: None,
                poll: None,
                quote_tweet_id: None,
                reply: None,
                reply_settings: None,
            };

            let res = self.client.post_tweet(tweet, Some(body_param)).await?;
            tracing::info!("Tweet response: {:?}", res);

            self.my_tweets
                .insert(res.data.id.clone(), tweet.to_string())?;

            Ok(res.data.id)
        } else {
            tracing::info!("Mock tweet: {}", tweet);

            Ok("mock_tweet_id".to_string())
        }
    }

    pub async fn get_mentions(&self, max_results: Option<u32>) -> Result<Vec<TweetInfo>> {
        if !self.use_twitter {
            return Ok(vec![]);
        }

        let user_info = self.user_info().await?;
        let user_id = user_info.id;

        let mut max_results = max_results.unwrap_or(10);
        max_results = max_results.min(100).max(5);

        let user_fields = vec![UserField::Id, UserField::Username];
        let expansions = vec![ExpansionType::InReplyToUserId];
        let tweet_fields = vec![
            TweetField::Id,
            TweetField::AuthorId,
            TweetField::ConversationId,
            TweetField::InReplyToUserId,
            TweetField::ReferencedTweets,
        ];
        let query_params = MentionsQueryParams {
            max_results: Some(max_results),
            pagination_token: None,
            since_id: None,
            until_id: None,
            end_time: None,
            expansions: Some(expansions),
            media_fields: None,
            place_fields: None,
            poll_fields: None,
            start_time: None,
            tweet_fields: Some(tweet_fields),
            user_fields: Some(user_fields),
        };
        let res = self
            .client
            .get_users_mentions(&user_id, Some(query_params))
            .await?;

        let mut mentions = vec![];
        for tweet in res.data {
            let id = tweet.id;
            let text = tweet.text;
            let author_id = tweet.author_id.unwrap_or_default();
            let conversation_id = tweet.conversation_id.unwrap_or_default();
            let in_reply_to_user_id = tweet.in_reply_to_user_id;
            let referenced_tweets = tweet.referenced_tweets;

            if self.fetched_tweets.get(&id)?.is_some() {
                continue;
            }

            let tweet_info = TweetInfo {
                id: id.clone(),
                text,
                author_id,
                conversation_id,
                in_reply_to_user_id,
                referenced_tweets,
            };
            self.fetched_tweets.insert(id, tweet_info.clone())?;
            mentions.push(tweet_info);
        }

        Ok(mentions)
    }

    pub async fn get_replies(&self) -> Result<Vec<Reply>> {
        if !self.use_twitter {
            return Ok(vec![]);
        }

        let mentions = self.get_mentions(Some(10)).await?;

        let user_info = self.user_info().await?;

        let mut replies = vec![];
        for mention in mentions {
            match (mention.in_reply_to_user_id, mention.referenced_tweets) {
                (Some(in_reply_to_user_id), Some(referenced_tweets)) => {
                    if in_reply_to_user_id == user_info.id {
                        let conversations = self.build_conversations(&mention.id).await?;
                        replies.push(Reply {
                            id: mention.id,
                            text: mention.text,
                            author_id: mention.author_id,
                            conversation_id: mention.conversation_id,
                            in_reply_to_user_id: in_reply_to_user_id,
                            referenced_tweets: referenced_tweets,
                            conversations: conversations,
                        });
                    }
                }
                _ => {}
            }
        }

        // TODO: check the tweet info

        Ok(replies)
    }

    async fn build_conversations(&self, id: &str) -> Result<Vec<String>> {
        let mut cur_tweet = self.get_tweet(id).await?;
        let username = self.get_user(&cur_tweet.author_id).await?.username;

        let conversation = format!("{}: {}", username, cur_tweet.text);
        let mut conversations = vec![conversation];
        while let Some(ref referenced_tweet) = cur_tweet.referenced_tweets {
            let parent_tweet_id = referenced_tweet
                .iter()
                .find(|tweet| tweet.r#type == "replied_to")
                .map(|tweet| tweet.id.clone());
            if let Some(parent_tweet_id) = parent_tweet_id {
                cur_tweet = self.get_tweet(&parent_tweet_id).await?;
                let username = self.get_user(&cur_tweet.author_id).await?.username;
                let conversation = format!("{}: {}", username, cur_tweet.text);
                conversations.push(conversation);
            }
        }

        conversations.reverse();

        Ok(conversations)
    }

    pub async fn reply_to(&self, id: &str, tweet: &str) -> Result<()> {
        if !self.use_twitter {
            tracing::info!("Mock reply to {}: {}", id, tweet);
            return Ok(());
        }

        tracing::info!("Replying to {}: {}", id, tweet);

        let user_info = self.user_info().await?;

        let body_param = PostTweetParams {
            direct_message_deep_link: None,
            for_super_followers_only: None,
            geo: None,
            media: None,
            poll: None,
            quote_tweet_id: None,
            reply: Some(ReplyParams {
                in_reply_to_tweet_id: Some(id.to_string()),
                exclude_reply_user_ids: None,
            }),
            reply_settings: None,
        };

        let res = self.client.post_tweet(tweet, Some(body_param)).await?;

        self.my_replies.insert(res.data.id, tweet.to_string())?;

        Ok(())
    }

    pub async fn get_tweet(&self, tweet_id: &str) -> Result<TweetInfo> {
        if !self.use_twitter {
            anyhow::bail!("Config is not set to use twitter");
        }

        if let Some(tweet) = self.fetched_tweets.get(&tweet_id.to_string())? {
            return Ok(tweet);
        }

        let user_fields = vec![UserField::Id, UserField::Username];
        let expansions = vec![ExpansionType::InReplyToUserId];
        let tweet_fields = vec![
            TweetField::Id,
            TweetField::AuthorId,
            TweetField::ConversationId,
            TweetField::InReplyToUserId,
            TweetField::ReferencedTweets,
        ];
        let query_params = TweetQueryParams {
            expansions: Some(expansions),
            media_fields: None,
            place_fields: None,
            poll_fields: None,
            tweet_fields: Some(tweet_fields),
            user_fields: Some(user_fields),
        };

        let res = self
            .client
            .get_tweet_info_with_params(tweet_id, Some(query_params))
            .await?;
        tracing::info!("Tweet: {:?}", res);

        let tweet_info = TweetInfo {
            id: res.data.id,
            text: res.data.text,
            author_id: res.data.author_id.unwrap_or_default(),
            conversation_id: res.data.conversation_id.unwrap_or_default(),
            in_reply_to_user_id: res.data.in_reply_to_user_id,
            referenced_tweets: res.data.referenced_tweets,
        };

        self.fetched_tweets
            .insert(tweet_id.to_string(), tweet_info.clone())?;

        Ok(tweet_info)
    }

    pub async fn get_user(&self, user_id: &str) -> Result<UserInfo> {
        if !self.use_twitter {
            anyhow::bail!("Config is not set to use twitter");
        }

        if let Some(user) = self.fetched_users.get(&user_id.to_string())? {
            return Ok(user);
        }

        let res = self.client.get_user_by_id(user_id, None).await?;
        tracing::info!("User: {:?}", res);

        let id = res
            .get("data")
            .ok_or(anyhow::anyhow!("No data found"))?
            .get("id")
            .ok_or(anyhow::anyhow!("No id found"))?
            .as_str()
            .ok_or(anyhow::anyhow!("Failed to parse id"))?;
        let name = res
            .get("data")
            .ok_or(anyhow::anyhow!("No data found"))?
            .get("name")
            .ok_or(anyhow::anyhow!("No name found"))?
            .as_str()
            .ok_or(anyhow::anyhow!("Failed to parse name"))?;
        let username = res
            .get("data")
            .ok_or(anyhow::anyhow!("No data found"))?
            .get("username")
            .ok_or(anyhow::anyhow!("No username found"))?
            .as_str()
            .ok_or(anyhow::anyhow!("Failed to parse username"))?;

        let user_info = UserInfo {
            id: id.to_string(),
            name: name.to_string(),
            username: username.to_string(),
        };

        self.fetched_users
            .insert(user_id.to_string(), user_info.clone())?;

        Ok(user_info)
    }
}

#[cfg(test)]
mod twitter_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_replies() -> Result<()> {
        crate::setup_env_and_tracing();

        let twitter_client = TwitterClient::get();

        let res = twitter_client.get_replies().await?;
        tracing::info!("Replies: {:?}", res);

        Ok(())
    }
}
