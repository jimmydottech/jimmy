use anyhow::Result;
use serde::{Deserialize, Serialize};

use std::fs::File;
use std::io::{BufReader, Read, Write};

use crate::config::Config;

const SGX_QUOTE_MAX_SIZE: usize = 8192 * 4;
const SGX_TARGET_INFO_SIZE: usize = 512;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Attestation {
    pub id: String,
    pub user_report: Vec<u8>,
    /// hex encoded quote
    pub quote: String,
}

pub fn generate_raw_attestation(user_report: &[u8]) -> Result<Attestation> {
    if user_report.len() > 64 {
        anyhow::bail!("user_report must not exceed 64 bytes");
    }

    let config = Config::get();
    let quote = if config.run_in_sgx {
        let mut target_info = vec![0; SGX_TARGET_INFO_SIZE];

        // read my_target_info to `target_info`
        let mut file = File::open("/dev/attestation/my_target_info")?;
        BufReader::new(file).read_exact(&mut target_info)?;

        file = File::create("/dev/attestation/target_info")?;
        file.write(&target_info)?;
        file = File::create("/dev/attestation/user_report_data")?;
        file.write(user_report)?;

        // read quote
        let mut quote = vec![0; SGX_QUOTE_MAX_SIZE];
        file = File::open("/dev/attestation/quote")?;
        BufReader::new(file).read(&mut quote)?;
        let real_len = quote.iter().rposition(|x| *x != 0);
        if real_len.is_none() {
            anyhow::bail!("quote without EOF");
        }

        let quote_hex = hex::encode(&quote[..=(real_len.unwrap() + 1)]);
        quote_hex
    } else {
        String::new()
    };

    let uuid = uuid::Uuid::new_v4().simple().to_string();

    let attestation = Attestation {
        id: uuid,
        user_report: user_report.to_vec(),
        quote,
    };

    Ok(attestation)
}
