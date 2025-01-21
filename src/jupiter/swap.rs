use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{signature::Signature, transaction::VersionedTransaction};

use super::SOL_MINT;
use crate::wallet::Wallet;
use crate::{client::get_http_client, config::Config};

// allow camelCase in struct fields
// Doc: https://serde.rs/field-attrs.html
#[derive(Serialize)]
struct SwapRequest<'a> {
    #[serde(rename = "quoteResponse")]
    quote_response: &'a serde_json::Value,
    #[serde(rename = "userPublicKey")]
    user_public_key: String,
    #[serde(rename = "wrapAndUnwrapSol")]
    wrap_and_unwrap_sol: bool,
    #[serde(rename = "prioritizationFeeLamports")]
    prioritization_fee_lamports: u64,
}

pub async fn swap_from_sol(output_mint: &str, amount: u64) -> Result<(u64, Signature)> {
    swap(SOL_MINT, output_mint, amount).await
}

pub async fn swap_to_sol(input_mint: &str, amount: u64) -> Result<(u64, Signature)> {
    swap(input_mint, SOL_MINT, amount).await
}

pub async fn swap(input_mint: &str, output_mint: &str, amount: u64) -> Result<(u64, Signature)> {
    let config = Config::get();
    let wallet = Wallet::get();
    let jupiter_rpc_url = &config.jupiter_rpc_url;

    let rpc_client = RpcClient::new(jupiter_rpc_url.into());

    // Define the quote API endpoint and parameters
    let quote_url = "https://quote-api.jup.ag/v6/quote";
    let amount_str = amount.to_string();
    let params = [
        ("inputMint", input_mint),
        ("outputMint", output_mint),
        ("amount", &amount_str),
        ("slippageBps", "300"), // 3% slippage
    ];

    let http_client = get_http_client();

    // Make GET request to fetch the quote
    // Doc: https://station.jup.ag/api-v6/get-quote
    let quote_response: serde_json::Value = http_client
        .get(quote_url)
        .query(&params)
        .send()
        .await?
        .json()
        .await?;

    let out_amount = quote_response
        .get("outAmount")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or(anyhow::anyhow!(
            "Failed to parse outAmount from the response"
        ))?;

    tracing::info!(
        "Swapping: inAmount {}, outAmount {}, inputMint {}, outputMint {}",
        amount,
        out_amount,
        input_mint,
        output_mint
    );

    if config.mock_trade {
        // tracing::info!("Mock trade enabled. Skipping swap.");
        return Ok((out_amount, Signature::default()));
    }

    let swap_url = "https://quote-api.jup.ag/v6/swap";
    let swap_request = SwapRequest {
        quote_response: &quote_response,
        user_public_key: wallet.pubkey().to_string(),
        wrap_and_unwrap_sol: true,
        prioritization_fee_lamports: 200000,
    };

    // Make POST request to fetch the swap transaction
    // Doc: https://station.jup.ag/api-v6/post-swap
    let swap_response: serde_json::Value = http_client
        .post(swap_url)
        .json(&swap_request)
        .send()
        .await?
        .json()
        .await?;

    tracing::info!("Swap response: {:?}", swap_response);

    // Extract the swapTransaction field from the response
    let swap_transaction_b64 = swap_response
        .get("swapTransaction")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("swapTransaction not found in the response"))?;

    // Decode the Base64-encoded transaction
    let swap_transaction_bytes = general_purpose::STANDARD
        .decode(swap_transaction_b64)
        .map_err(|_| anyhow::anyhow!("Failed to decode swapTransaction from Base64"))?;

    // Deserialize into a VersionedTransaction
    let versioned_tx: VersionedTransaction = bincode::deserialize(&swap_transaction_bytes)
        .map_err(|_| anyhow::anyhow!("Failed to deserialize swapTransaction"))?;

    let signed_versioned_tx =
        VersionedTransaction::try_new(versioned_tx.message, &[wallet.keypair()]).unwrap();

    // Send the transaction to the Solana network
    let txid = rpc_client
        .send_and_confirm_transaction(&signed_versioned_tx)
        .await?;

    tracing::info!(
        "Transaction confirmed. View on Solscan: https://solscan.io/tx/{}",
        txid
    );

    Ok((out_amount, txid))
}
