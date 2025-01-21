use anyhow::Result;
use serde_json::Value;
use solana_account_decoder::{parse_token::UiTokenAmount, UiAccountData};
use solana_client::{
    rpc_config::RpcSendTransactionConfig, rpc_request::TokenAccountsFilter,
    rpc_response::RpcKeyedAccount,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    pubkey::Pubkey,
    signer::{keypair::Keypair, EncodableKey, Signer},
    system_instruction,
    transaction::Transaction,
};

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::Duration;

use crate::client::get_finalized_client;
use crate::config::Config;
use crate::constant::*;
use crate::store::{LocalStore, Store};
use crate::token::structs::TokenAccount;

pub struct Wallet {
    is_new: bool,
    keypair: Keypair,
}

impl Wallet {
    fn new() -> Self {
        const KEY_NAME: &str = "Wallet_keypair";

        let mut is_new = true;
        let keypair = if let Some(value) =
            LocalStore::get(KEY_NAME.as_bytes()).expect("Failed to get wallet from store")
        {
            let keypair = Keypair::from_bytes(&value).expect("Failed to recover wallet");
            tracing::info!("Wallet recovered from store: {}", keypair.pubkey());
            is_new = false;
            keypair
        } else {
            let keypair = Keypair::new();
            let value = keypair.to_bytes();
            LocalStore::put(KEY_NAME.as_bytes(), &value).expect("Failed to save wallet");
            tracing::info!("Wallet saved to store: {}", keypair.pubkey());
            keypair
        };

        // write keypair to path
        let path = Self::path();
        keypair
            .write_to_file(&path)
            .expect("Failed to write keypair to file");

        Self { keypair, is_new }
    }

    pub fn get() -> &'static Self {
        static INSTANCE: OnceLock<Wallet> = OnceLock::new();
        INSTANCE.get_or_init(Self::new)
    }

    pub fn path() -> PathBuf {
        let config = Config::get();
        let path = Path::new(&config.store_path);
        path.join("Wallet_keypair")
    }

    pub fn is_new(&self) -> bool {
        self.is_new
    }

    pub fn balance(&self) -> Result<u64> {
        let client = get_finalized_client();
        Ok(client.get_balance(&self.keypair.pubkey())?)
    }

    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    pub async fn get_airdrop(&self, amount: u64) -> Result<()> {
        let client = get_finalized_client();
        let airdrop_signature = client.request_airdrop(&self.pubkey(), amount)?;

        tracing::info!("Airdrop {} SOL requested", amount / LAMPORTS_PER_SOL);

        tokio::time::timeout(Duration::from_secs(30), async {
            while let Ok(false) = client.confirm_transaction(&airdrop_signature) {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .await?;

        tracing::info!("Airdrop confirmed");

        Ok(())
    }

    pub(crate) fn keypair(&self) -> &Keypair {
        &self.keypair
    }

    pub async fn get_token_balance(&self, token_mint: &Pubkey) -> Result<UiTokenAmount> {
        let client = get_finalized_client();
        let token_accounts = client.get_token_accounts_by_owner(
            &self.pubkey(),
            TokenAccountsFilter::Mint(token_mint.clone()),
        )?;

        if let Some(token_account_info) = token_accounts.first() {
            let pubkey = Pubkey::from_str(&token_account_info.pubkey)?;
            let token_account: UiTokenAmount = client.get_token_account_balance(&pubkey)?;
            Ok(token_account)
        } else {
            Err(anyhow::anyhow!("No token account found"))
        }
    }

    pub async fn get_token_info(&self, token_mint: &Pubkey) -> Result<TokenAccount> {
        let client = get_finalized_client();
        let token_accounts = client.get_token_accounts_by_owner(
            &self.pubkey(),
            TokenAccountsFilter::Mint(token_mint.clone()),
        )?;

        if let Some(token_account_info) = token_accounts.first() {
            parse_token_account(token_account_info.clone())
        } else {
            Err(anyhow::anyhow!("No token account found"))
        }
    }

    pub async fn get_all_tokens_info(&self) -> Result<Vec<TokenAccount>> {
        let client = get_finalized_client();

        let token_accounts = client.get_token_accounts_by_owner(
            &self.pubkey(),
            TokenAccountsFilter::ProgramId(spl_token::id()),
        )?;

        token_accounts
            .into_iter()
            .map(|token_account_info| parse_token_account(token_account_info))
            .collect()
    }

    pub async fn wsol_balance(&self) -> Result<u64> {
        let wsol_mint = spl_token::native_mint::id();
        let balance = self
            .get_token_balance(&wsol_mint)
            .await?
            .amount
            .parse::<u64>()?;
        Ok(balance)
    }

    pub fn create_and_fund_wsol_ata(&self, amount: u64) -> Result<Pubkey> {
        let wsol_mint = spl_token::native_mint::id();
        let ata =
            spl_associated_token_account::get_associated_token_address(&self.pubkey(), &wsol_mint);

        let client = get_finalized_client();

        let priority_ix = ComputeBudgetInstruction::set_compute_unit_price(COMPUTE_UNIT_PRICE);
        let mut instructions = vec![priority_ix];

        // Only add create instruction if the account doesn't exist
        match client.get_account(&ata) {
            Ok(_) => {
                tracing::info!("WSOL ATA already exists: {}", ata);
            }
            Err(_) => {
                instructions.push(
                    spl_associated_token_account::instruction::create_associated_token_account(
                        &self.pubkey(),
                        &self.pubkey(),
                        &wsol_mint,
                        &spl_token::id(),
                    ),
                );
                tracing::info!("Creating new WSOL ATA: {}", ata);
            }
        }

        // Always add transfer and sync instructions
        instructions.extend([
            // Transfer SOL to wrap into WSOL
            system_instruction::transfer(&self.pubkey(), &ata, amount),
            // Sync wrapped SOL balance
            spl_token::instruction::sync_native(&spl_token::id(), &ata)?,
        ]);

        for retry in 0..OUTER_MAX_RETRIES {
            let recent_blockhash = client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &instructions,
                Some(&self.pubkey()),
                &[&self.keypair],
                recent_blockhash,
            );

            let signature = match client.send_and_confirm_transaction_with_spinner_and_config(
                &transaction,
                CommitmentConfig::finalized(),
                RpcSendTransactionConfig {
                    skip_preflight: SKIP_PREFLIGHT,
                    max_retries: Some(INNER_MAX_RETRIES),
                    ..RpcSendTransactionConfig::default()
                },
            ) {
                Ok(signature) => signature,
                Err(e) => {
                    tracing::warn!("Error: {}", e);
                    if retry == OUTER_MAX_RETRIES - 1 {
                        tracing::error!(
                            "Failed to create and fund WSOL ATA after {} retries",
                            OUTER_MAX_RETRIES
                        );
                        anyhow::bail!("Failed to create and fund WSOL ATA");
                    } else {
                        tracing::info!("Retrying to create and fund WSOL ATA...");
                        continue;
                    }
                }
            };

            tracing::info!("Created and funded WSOL ATA: {}", ata);

            return Ok(ata);
        }

        anyhow::bail!("Failed to create and fund WSOL ATA")
    }
}

// RpcKeyedAccount example:
// RpcKeyedAccount {
//     pubkey: "7pr6SSrACFm7wBrDCkVpkzkLftsEamexnc1pZB7cMER",
//     account: UiAccount {
//         lamports: 2039280,
//         data: Json(ParsedAccount {
//             program: "spl-token",
//             parsed: Object {
//                 "info": Object {
//                     "isNative": Bool(false),
//                     "mint": String("H9U1nxAtT12mPd8vHLTLvBsBqhWUGy7bg1KqDLaSMLNk"),
//                     "owner": String("6TguS5yoiy7Adnfs3Egk5YZmx4jGYagdrnTJuRuRrXkt"),
//                     "state": String("initialized"),
//                     "tokenAmount": Object {
//                         "amount": String("1000000000000000000"),
//                         "decimals": Number(9),
//                         "uiAmount": Number(1000000000.0),
//                         "uiAmountString": String("1000000000"),
//                     }
//                 },
//                 "type": String("account"),
//             },
//             space: 165
//         }),
//         owner: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
//         executable: false,
//         rent_epoch: 18446744073709551615,
//         space: Some(165)
//     }
// }
fn parse_token_account(rpc_keyed_account: RpcKeyedAccount) -> Result<TokenAccount> {
    let pubkey = Pubkey::from_str(&rpc_keyed_account.pubkey)?;

    let account_data: UiAccountData = rpc_keyed_account.account.data;
    if let UiAccountData::Json(parsed_account) = account_data {
        let parsed = parsed_account.parsed;
        let info = parsed
            .get("info")
            .ok_or_else(|| anyhow::anyhow!("Missing info"))?;

        let mint = info
            .get("mint")
            .and_then(|v| v.as_str())
            .and_then(|v| Pubkey::from_str(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Failed to parse mint"))?;
        let owner = info
            .get("owner")
            .and_then(|v| v.as_str())
            .and_then(|v| Pubkey::from_str(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Failed to parse owner"))?;

        let token_amount = info
            .get("tokenAmount")
            .ok_or_else(|| anyhow::anyhow!("Failed to parse tokenAmount"))?;

        let raw_amount = token_amount
            .get("amount")
            .and_then(|v| v.as_str())
            .and_then(|v| u64::from_str(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Failed to parse amount"))?;

        let decimals = token_amount
            .get("decimals")
            .and_then(|v| v.as_u64())
            .and_then(|v| u8::try_from(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Failed to parse decimals"))?;

        let ui_amount = token_amount
            .get("uiAmount")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| anyhow::anyhow!("Failed to parse uiAmount"))?;

        return Ok(TokenAccount {
            pubkey,
            mint,
            owner,
            raw_amount,
            decimals,
            ui_amount,
        });
    }

    Err(anyhow::anyhow!("Failed to parse token account"))
}
