use anyhow::Result;
use async_openai::{
    config::AzureConfig,
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs},
    Client,
};

use std::sync::OnceLock;

use crate::config::Config;

pub fn azure_client() -> &'static Client<AzureConfig> {
    static CLIENT: OnceLock<Client<AzureConfig>> = OnceLock::new();

    CLIENT.get_or_init(|| {
        let config = AzureConfig::new()
            .with_api_base(Config::get().azure_openai_endpoint.clone())
            .with_api_key(Config::get().azure_openai_api_key.clone())
            .with_api_version(Config::get().azure_openai_api_version.clone())
            .with_deployment_id(Config::get().azure_openai_chat_model.clone());

        Client::with_config(config)
    })
}

pub async fn run_prompt(prompt: impl AsRef<str>) -> Result<String> {
    let model = &Config::get().azure_openai_chat_model;
    let req = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages([ChatCompletionRequestMessageArgs::default()
            .content(prompt.as_ref())
            .build()?
            .into()])
        .build()?;

    let resp = azure_client().chat().create(req).await?;

    Ok(resp
        .choices
        .first()
        .ok_or(anyhow::anyhow!("No response from LLM"))?
        .message
        .content
        .clone()
        .ok_or(anyhow::anyhow!("No content in response from LLM"))?)
}
