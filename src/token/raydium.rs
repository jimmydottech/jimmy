use anyhow::Result;
use clmm_cli::ClmmCommands;
use common::CommonConfig;
use common::{common_types, rpc};
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

use std::str::FromStr;
use std::sync::Arc;

use crate::client::get_finalized_client;
use crate::constant::*;
use crate::token::jimmy::JimmyToken;
use crate::wallet::Wallet;

/// Creates a Raydium Concentrated Liquidity Market Maker (CLMM) pool for JIMMY/SOL pair
///
/// The relative_price parameter represents mint0/mint1 ratio (JIMMY/SOL).
/// For example, if relative_price = 0.04:
/// - 25 JIMMY = 1 SOL
/// - 1 JIMMY = 0.04 SOL
pub async fn create_raydium_clmm_pool(
    jimmy_token: &JimmyToken,
    relative_price: f64,
) -> Result<Pubkey> {
    let sol_mint = Pubkey::from_str(SOL_MINT)?;
    let amm_config = get_amm_config_pubkey().await?;

    #[cfg(feature = "devnet")]
    let command = ClmmCommands::CreatePool {
        mint0: jimmy_token.mint_pubkey(),
        mint1: sol_mint,
        amm_config,
        price: relative_price,
        open_time: 0,
        nonce: 1,
    };
    #[cfg(not(feature = "devnet"))]
    let command = ClmmCommands::CreatePool {
        mint0: jimmy_token.mint_pubkey(),
        mint1: sol_mint,
        amm_config,
        price: relative_price,
        open_time: 0,
    };

    execute_command(command)?;

    let common_config = get_common_config();
    let pool_id = generate_pool_id(
        &common_config,
        &amm_config,
        &jimmy_token.mint_pubkey(),
        &sol_mint,
    );
    tracing::info!("Create Raydium CLMM Pool, Pool ID: {}", pool_id);
    Ok(pool_id)
}

pub fn fetch_pool(jimmy_token: &JimmyToken) -> Result<()> {
    let sol_mint = Pubkey::from_str(SOL_MINT)?;

    let mut mint0 = jimmy_token.mint_pubkey();
    let mut mint1 = sol_mint;
    if mint0 > mint1 {
        std::mem::swap(&mut mint0, &mut mint1);
    }

    let command = ClmmCommands::FetchPool {
        pool_id: None,
        mint0: Some(mint0),
        mint1: Some(mint1),
    };

    execute_command(command)
}

pub fn create_position(
    jimmy_token: &JimmyToken,
    sol_amount: u64,
    mut lower_price: f64,
    mut upper_price: f64,
) -> Result<()> {
    let sol_mint = Pubkey::from_str(SOL_MINT)?;

    let mut mint0 = jimmy_token.mint_pubkey();
    let mut mint1 = sol_mint;
    let mut base_token1 = true;
    if mint0 > mint1 {
        std::mem::swap(&mut mint0, &mut mint1);
        lower_price = 1.0 / lower_price;
        upper_price = 1.0 / upper_price;
        base_token1 = !base_token1;
    }
    if upper_price < lower_price {
        std::mem::swap(&mut lower_price, &mut upper_price);
    }

    let pool_id = jimmy_token
        .pool_id()
        .ok_or(anyhow::anyhow!("Pool ID not found"))?;

    let command = ClmmCommands::OpenPosition {
        pool_id: pool_id,
        deposit_token0: None,
        deposit_token1: None,
        tick_lower_price: lower_price,
        tick_upper_price: upper_price,
        amount_specified: sol_amount,
        base_token1: base_token1,
        without_metadata: false,
        traditional_nft: false,
    };

    execute_command(command)?;

    tracing::info!("Create Raydium CLMM position success");

    Ok(())
}

// TODO: Fix raydium-library dependency
// increase_liquidity is not working because raydium-library cannot retrieve position information due to a bug.
pub fn increase_liquidity(
    jimmy_token: &JimmyToken,
    amount: u64,
    mut lower_price: f64,
    mut upper_price: f64,
) -> Result<()> {
    let sol_mint = Pubkey::from_str(SOL_MINT)?;

    let mut mint0 = jimmy_token.mint_pubkey();
    let mut mint1 = sol_mint;
    let mut base_token1 = false;
    if mint0 > mint1 {
        std::mem::swap(&mut mint0, &mut mint1);
        lower_price = 1.0 / lower_price;
        upper_price = 1.0 / upper_price;
        base_token1 = true;
    }

    let pool_id = jimmy_token
        .pool_id()
        .ok_or(anyhow::anyhow!("Pool ID not found"))?;

    let command = ClmmCommands::IncreaseLiquidity {
        pool_id: pool_id,
        deposit_token0: None,
        deposit_token1: None,
        tick_lower_price: lower_price,
        tick_upper_price: upper_price,
        amount_specified: amount,
        base_token1: base_token1,
    };

    execute_command(command)?;

    tracing::info!("Increase Raydium CLMM liquidity success");

    Ok(())
}

pub fn execute_command(command: ClmmCommands) -> Result<()> {
    tracing::info!("Executing CLMM command: {:?}", command);

    let config = get_common_config();

    let wallet = Wallet::get();
    let mut signing_keypairs: Vec<Arc<dyn Signer>> = Vec::new();
    // TODO: use Arc<Keypair>
    signing_keypairs.push(Arc::new(wallet.keypair().insecure_clone()));

    let instructions = clmm_cli::process_clmm_commands(command, &config, &mut signing_keypairs)?;
    tracing::debug!("Instructions: {:#?}", instructions);

    if let Some(mut instructions) = instructions {
        let priority_ix = ComputeBudgetInstruction::set_compute_unit_price(COMPUTE_UNIT_PRICE);
        instructions.insert(0, priority_ix);

        for retry in 0..OUTER_MAX_RETRIES {
            let rpc_client = get_finalized_client();
            let txn = rpc::build_txn(
                &rpc_client,
                &instructions,
                &wallet.pubkey(),
                &signing_keypairs,
            )?;

            let sig = match rpc_client.send_and_confirm_transaction_with_spinner_and_config(
                &txn,
                CommitmentConfig::confirmed(),
                RpcSendTransactionConfig {
                    skip_preflight: SKIP_PREFLIGHT,
                    max_retries: Some(INNER_MAX_RETRIES),
                    ..RpcSendTransactionConfig::default()
                },
            ) {
                Ok(sig) => sig,
                Err(e) => {
                    tracing::warn!("Failed to send transaction: {}", e);
                    if retry == OUTER_MAX_RETRIES - 1 {
                        anyhow::bail!(
                            "Failed to send transaction after {} retries",
                            OUTER_MAX_RETRIES
                        );
                    }
                    tracing::info!("Retrying to send transaction...");
                    continue;
                }
            };
            // let sig = rpc::send_txn(&rpc_client, &txn, true)?;
            tracing::info!("CLMM command transaction sent: {:#?}", sig);
            return Ok(());
        }
    } else {
        tracing::info!("No instructions needed to execute");
    }

    Ok(())
}

pub fn get_common_config() -> CommonConfig {
    let mut config = common_types::CommonConfig::default();
    config.set_wallet(Wallet::path().to_str().expect("Failed to get wallet path"));
    config
}

#[cfg(not(feature = "devnet"))]
pub fn generate_pool_id(
    common_config: &CommonConfig,
    amm_config: &Pubkey,
    mint_0: &Pubkey,
    mint_1: &Pubkey,
) -> Pubkey {
    let mut mint0 = *mint_0;
    let mut mint1 = *mint_1;
    if mint0 > mint1 {
        std::mem::swap(&mut mint0, &mut mint1);
    }

    let program_id = common_config.clmm_program();
    let (addr, _) = Pubkey::find_program_address(
        &[
            "pool".as_bytes(),
            amm_config.as_ref(),
            mint0.as_ref(),
            mint1.as_ref(),
        ],
        &program_id,
    );

    addr
}

#[cfg(feature = "devnet")]
pub fn generate_pool_id(
    common_config: &CommonConfig,
    amm_config: &Pubkey,
    mint_0: &Pubkey,
    mint_1: &Pubkey,
) -> Pubkey {
    let mut mint0 = *mint_0;
    let mut mint1 = *mint_1;
    if mint0 > mint1 {
        std::mem::swap(&mut mint0, &mut mint1);
    }

    let nonce = 1u8;
    let program_id = common_config.clmm_program();
    let (addr, _) = Pubkey::find_program_address(
        &[
            "pool".as_bytes(),
            amm_config.as_ref(),
            mint0.as_ref(),
            mint1.as_ref(),
            &nonce.to_be_bytes(),
        ],
        &program_id,
    );

    addr
}

pub async fn get_amm_config_pubkey() -> Result<Pubkey> {
    let amm_config = get_amm_config().await?;
    if amm_config.is_empty() {
        return Err(anyhow::anyhow!("amm config is empty"));
    }

    let pubkey = Pubkey::from_str(
        amm_config[0]
            .get("id")
            .ok_or(anyhow::anyhow!("Invalid amm config"))?
            .as_str()
            .ok_or(anyhow::anyhow!("Invalid amm config"))?,
    )?;
    Ok(pubkey)
}

#[cfg(not(feature = "devnet"))]
pub async fn get_amm_config() -> anyhow::Result<Vec<serde_json::Value>> {
    use crate::client::get_http_client;

    let url = "https://api-v3.raydium.io/main/clmm-config";
    let response = get_http_client().get(url).send().await?;

    let json = response.json::<serde_json::Value>().await?;

    let data = json["data"]
        .as_array()
        .ok_or(anyhow::anyhow!("Invalid data"))?;

    Ok(data.clone())
}

#[cfg(feature = "devnet")]
pub async fn get_amm_config() -> Result<Vec<serde_json::Value>> {
    Ok(vec![
        serde_json::json!({
            "id": "CQYbhr6amxUER4p5SC44C63R4qw4NFc9Z4Db9vF4tZwG",
            "index": 0,
            "protocolFeeRate": 120000,
            "tradeFeeRate": 100,
            "tickSpacing": 10,
            "fundFeeRate": 40000,
            "description": "Best for very stable pairs",
            "defaultRange": 0.005,
            "defaultRangePoint": [0.001, 0.003, 0.005, 0.008, 0.01]
        }),
        serde_json::json!({
            "id": "B9H7TR8PSjJT7nuW2tuPkFC63z7drtMZ4LoCtD7PrCN1",
            "index": 1,
            "protocolFeeRate": 120000,
            "tradeFeeRate": 2500,
            "tickSpacing": 60,
            "fundFeeRate": 40000,
            "description": "Best for most pairs",
            "defaultRange": 0.1,
            "defaultRangePoint": [0.01, 0.05, 0.1, 0.2, 0.5]
        }),
        serde_json::json!({
            "id": "GjLEiquek1Nc2YjcBhufUGFRkaqW1JhaGjsdFd8mys38",
            "index": 3,
            "protocolFeeRate": 120000,
            "tradeFeeRate": 10000,
            "tickSpacing": 120,
            "fundFeeRate": 40000,
            "description": "Best for exotic pairs",
            "defaultRange": 0.1,
            "defaultRangePoint": [0.01, 0.05, 0.1, 0.2, 0.5]
        }),
        serde_json::json!({
            "id": "GVSwm4smQBYcgAJU7qjFHLQBHTc4AdB3F2HbZp6KqKof",
            "index": 2,
            "protocolFeeRate": 120000,
            "tradeFeeRate": 500,
            "tickSpacing": 10,
            "fundFeeRate": 40000,
            "description": "Best for tighter ranges",
            "defaultRange": 0.1,
            "defaultRangePoint": [0.01, 0.05, 0.1, 0.2, 0.5]
        }),
    ])
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_raydium_clmm_pool() {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();
    let _ = create_raydium_clmm_pool(JimmyToken::get().await, 25.0).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn get_multi_accounts() {
    dotenv::dotenv().ok();

    let client = get_finalized_client();
    let pubkey = Pubkey::from_str("7YW28zsnYcM3Cmtnr74vH2NWxsW2CaCmYSeCfZfLRJSN").unwrap();

    let t = client.get_multiple_accounts(&[pubkey]).unwrap();

    println!("{:?}", t);
}
