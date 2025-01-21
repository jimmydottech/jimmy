use std::{sync::OnceLock, time::Duration};

use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

use crate::config::Config;

const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

pub fn get_finalized_client() -> RpcClient {
    let url = Config::get().solana_rpc_url.clone();
    RpcClient::new_with_timeout_and_commitment(url, CLIENT_TIMEOUT, CommitmentConfig::finalized())
}

pub fn get_confirmed_client() -> RpcClient {
    let url = Config::get().solana_rpc_url.clone();
    RpcClient::new_with_timeout_and_commitment(url, CLIENT_TIMEOUT, CommitmentConfig::confirmed())
}

pub fn get_http_client() -> &'static reqwest::Client {
    static INSTANCE: OnceLock<reqwest::Client> = OnceLock::new();
    INSTANCE.get_or_init(|| reqwest::Client::new())
}
