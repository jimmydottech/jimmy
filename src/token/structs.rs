/// # Token Amounts
///
/// Using `u64` to represent token amounts and balances.
///
/// In the Solana blockchain, the smallest unit of SOL is called a lamport, where 1 SOL = 1,000,000,000 lamports.
/// For most token balances, including SOL and other SPL tokens, using `u64` is sufficient because:
/// - The maximum value of `u64` (18,446,744,073,709,551,615) can comfortably accommodate the total supply of SOL,
///   which is capped at around 511 million SOL (or 511,000,000,000 lamports).
/// - Token balances are typically well within the range of `u64`, making it a suitable choice for representing
///   both raw amounts in lamports and user-friendly amounts in SOL or other token units.
///
/// If there is a need to handle extremely large numbers or perform complex calculations that could exceed
/// the limits of `u64`, consider using `u128`. However, for standard token balances, `u64` is adequate.
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAccount {
    pub pubkey: Pubkey,
    pub mint: Pubkey,
    pub owner: Pubkey,   // the wallet address that owns the token account
    pub raw_amount: u64, // e.g. 1000000000000000000
    pub decimals: u8,    // e.g. 9
    pub ui_amount: f64,  // e.g. 1000000000.0
}

impl TokenAccount {
    pub fn one_token_amount(&self) -> u64 {
        10_u64.pow(self.decimals as u32)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct TokenInfo {
    pub address: Pubkey,
    pub decimals: u8,
    pub name: String,
    pub symbol: String,
    pub coingecko_id: Option<String>,
}

impl std::fmt::Display for TokenInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TokenInfo {{ address: {}, decimals: {}, name: {}, symbol: {}, coingecko_id: {} }}",
            self.address,
            self.decimals,
            self.name,
            self.symbol,
            self.coingecko_id.as_deref().unwrap_or("None")
        )
    }
}
