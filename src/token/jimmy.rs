use anyhow::Result;
use mpl_token_metadata::instructions as mpl_instruction;
use mpl_token_metadata::types::DataV2;
use serde::{Deserialize, Serialize};
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    program_pack::Pack,
    pubkey::Pubkey,
    signer::{keypair::Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_associated_token_account::instruction as ata_instruction;
use spl_token::state::Mint;
use tokio::sync::OnceCell;

use super::raydium::{create_position, create_raydium_clmm_pool};
use crate::client::get_finalized_client;
use crate::config::Config;
use crate::constant::*;
use crate::store::{LocalStore, Store};
use crate::token::utils::get_metadata;
use crate::wallet::Wallet;

#[derive(Debug)]
pub struct JimmyToken {
    mint: Keypair,
    owner_token_account: Pubkey,
    wallet_pubkey: Pubkey,
    raydium_pool_id: Option<Pubkey>,
}

impl JimmyToken {
    pub async fn get() -> &'static Self {
        static INSTANCE: OnceCell<JimmyToken> = OnceCell::const_new();
        INSTANCE
            .get_or_init(|| async {
                Self::recover_or_launch(&Wallet::get())
                    .await
                    .expect("Failed to launch JimmyToken")
            })
            .await
    }

    pub fn mint_pubkey(&self) -> Pubkey {
        self.mint.pubkey()
    }

    pub fn token_pubkey(&self) -> Pubkey {
        self.owner_token_account
    }

    pub fn wallet_pubkey(&self) -> Pubkey {
        self.wallet_pubkey
    }

    pub fn one_jimmy() -> u64 {
        let config = Config::get();
        10_u64.pow(config.token_decimals as u32)
    }

    pub fn pool_id(&self) -> Option<Pubkey> {
        self.raydium_pool_id
    }

    async fn recover_or_launch(wallet: &Wallet) -> Result<Self> {
        const KEY_NAME: &str = "JimmyToken";

        let mut jimmy_token = if let Some(value) = LocalStore::get(KEY_NAME.as_bytes())? {
            let jimmy_token_data: JimmyTokenData = bincode::deserialize(&value)?;
            let jimmy_token = JimmyToken::try_from(jimmy_token_data)?;
            tracing::info!("JimmyToken recovered from store");
            jimmy_token
        } else {
            let jimmy_token = Self::launch(wallet)?;
            let jimmy_token_data: JimmyTokenData = (&jimmy_token).into();
            let value = bincode::serialize(&jimmy_token_data)?;
            LocalStore::put(KEY_NAME.as_bytes(), &value)?;
            tracing::info!("JimmyToken launched and saved to store");
            jimmy_token
        };

        if jimmy_token.raydium_pool_id.is_none() {
            let config = Config::get();
            let pool_id = create_raydium_clmm_pool(&jimmy_token, config.raydium_pool_price).await?;
            jimmy_token.raydium_pool_id = Some(pool_id);

            let wallet = Wallet::get();
            let sol_amount = (config.raydium_pool_deposit * LAMPORTS_PER_SOL as f64) as u64;
            tracing::info!(
                "Creating WSOL ATA and funding with {} SOL",
                config.raydium_pool_deposit
            );
            wallet.create_and_fund_wsol_ata(sol_amount)?;
            let wsol_balance = wallet.wsol_balance().await?;
            tracing::info!("WSOL Balance: {}", wsol_balance);

            let lower_price = config.raydium_pool_min_price;
            let upper_price = config.raydium_pool_max_price;
            create_position(&jimmy_token, sol_amount, lower_price, upper_price)?;

            let jimmy_token_data: JimmyTokenData = (&jimmy_token).into();
            let value = bincode::serialize(&jimmy_token_data)?;
            LocalStore::put(KEY_NAME.as_bytes(), &value)?;
        }

        Ok(jimmy_token)
    }

    fn launch(wallet: &Wallet) -> Result<Self> {
        let mint = Keypair::new();
        let owner_token_account = spl_associated_token_account::get_associated_token_address(
            &wallet.pubkey(),
            &mint.pubkey(),
        );
        let wallet_pubkey = wallet.pubkey();

        let config = Config::get();
        let client = get_finalized_client();

        let mint_rent = client.get_minimum_balance_for_rent_exemption(Mint::LEN)?;
        let create_mint_ix: Instruction = system_instruction::create_account(
            &wallet_pubkey,
            &mint.pubkey(),
            mint_rent,
            Mint::LEN as u64,
            &spl_token::id(),
        );

        let initialize_mint_ix: Instruction = spl_token::instruction::initialize_mint(
            &spl_token::id(),
            &mint.pubkey(),
            &wallet_pubkey,
            None,
            config.token_decimals,
        )?;

        let create_ata_ix = ata_instruction::create_associated_token_account(
            &wallet_pubkey,
            &wallet_pubkey,
            &mint.pubkey(),
            &spl_token::id(),
        );

        let amount = config.total_supply * 10u64.pow(config.token_decimals as u32);
        let mint_to_ix = spl_token::instruction::mint_to(
            &spl_token::id(),
            &mint.pubkey(),
            &owner_token_account,
            &wallet_pubkey,
            &[],
            amount,
        )?;

        let (metadata_account, _) = Pubkey::find_program_address(
            &[
                b"metadata",
                mpl_token_metadata::ID.as_ref(),
                mint.pubkey().as_ref(),
            ],
            &mpl_token_metadata::ID,
        );

        let data = DataV2 {
            name: config.token_name.to_string(),
            symbol: config.token_symbol.to_string(),
            uri: config.token_uri.to_string(),
            seller_fee_basis_points: 0,
            creators: None,
            collection: None,
            uses: None,
        };

        let create_metadata_ix = mpl_instruction::CreateMetadataAccountV3Builder::new()
            .metadata(metadata_account)
            .mint(mint.pubkey())
            .mint_authority(wallet_pubkey)
            .payer(wallet_pubkey)
            .update_authority(wallet_pubkey, true)
            .data(data)
            .is_mutable(true) // TODO: set to false when metadata is fixed
            .instruction();

        let priority_ix = ComputeBudgetInstruction::set_compute_unit_price(COMPUTE_UNIT_PRICE);
        let instructions = vec![
            priority_ix,
            create_mint_ix,
            initialize_mint_ix,
            create_ata_ix,
            mint_to_ix,
            create_metadata_ix,
        ];

        for retry in 0..OUTER_MAX_RETRIES {
            // Combine all instructions into one transaction
            let recent_blockhash = client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &instructions,
                Some(&wallet_pubkey),
                &[wallet.keypair(), &mint],
                recent_blockhash,
            );

            tracing::info!("Launching JIMMY token...");
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
                            "Failed to launch JIMMY token after {} retries",
                            OUTER_MAX_RETRIES
                        );
                        anyhow::bail!("Failed to launch JIMMY token");
                    } else {
                        tracing::info!("Retrying to launch JIMMY token...");
                        continue;
                    }
                }
            };

            tracing::info!("🚀 JIMMY Token launched successfully!");
            tracing::info!("Mint Address: {}", mint.pubkey());
            tracing::info!("Your Token Account: {}", owner_token_account);
            tracing::info!("Transaction: {}", signature);

            let jimmy_token = Self {
                mint,
                owner_token_account,
                wallet_pubkey,
                raydium_pool_id: None,
            };

            return Ok(jimmy_token);
        }

        anyhow::bail!("Failed to launch JIMMY token")
    }

    pub fn print_balance(&self) -> Result<()> {
        let client = get_finalized_client();

        let balance = client.get_token_account_balance(&self.owner_token_account)?;
        tracing::info!(
            "JIMMY Token Balance: {} JIMMY",
            balance.ui_amount.unwrap_or_default()
        );

        Ok(())
    }

    pub fn print_metadata(&self) -> Result<()> {
        let metadata = get_metadata(self.mint_pubkey())?;

        tracing::info!("📝 Token Metadata:");
        tracing::info!("Name: {}", metadata.name);
        tracing::info!("Symbol: {}", metadata.symbol);
        tracing::info!("Mint: {}", metadata.mint);
        tracing::info!("Token Account: {}", self.owner_token_account);

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JimmyTokenData {
    mint: Vec<u8>,
    owner_token_account: Pubkey,
    wallet_pubkey: Pubkey,
    raydium_pool_id: Option<Pubkey>,
}

impl From<&JimmyToken> for JimmyTokenData {
    fn from(token: &JimmyToken) -> Self {
        Self {
            mint: token.mint.to_bytes().to_vec(),
            owner_token_account: token.owner_token_account,
            wallet_pubkey: token.wallet_pubkey,
            raydium_pool_id: token.raydium_pool_id,
        }
    }
}

impl TryFrom<JimmyTokenData> for JimmyToken {
    type Error = anyhow::Error;

    fn try_from(data: JimmyTokenData) -> Result<Self, Self::Error> {
        Ok(Self {
            mint: Keypair::from_bytes(&data.mint)?,
            owner_token_account: data.owner_token_account,
            wallet_pubkey: data.wallet_pubkey,
            raydium_pool_id: data.raydium_pool_id,
        })
    }
}
