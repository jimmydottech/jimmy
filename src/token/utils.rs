use anyhow::Result;
use mpl_token_metadata::accounts::Metadata;
use solana_sdk::pubkey::Pubkey;

use crate::client::get_finalized_client;

pub fn get_metadata(mint: Pubkey) -> Result<Metadata> {
    let client = get_finalized_client();

    let (metadata_account, _) = Pubkey::find_program_address(
        &[b"metadata", mpl_token_metadata::ID.as_ref(), mint.as_ref()],
        &mpl_token_metadata::ID,
    );

    let metadata_account_data = client.get_account_data(&metadata_account)?;
    let metadata = Metadata::from_bytes(&metadata_account_data)?;

    Ok(metadata)
}
