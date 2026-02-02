use std::sync::Arc;

use anyhow::Result;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::solana_zk_sdk::encryption::{auth_encryption::AeKey, elgamal::ElGamalKeypair};

use crate::solana::confidential_keys::ConfidentialKeys;

pub fn kp_from_base58_string(kp: &str) -> Keypair {
    Keypair::from_base58_string(kp)
}

pub fn el_gamal_deterministic(kp: &dyn Signer) -> Result<ElGamalKeypair> {
    let message = b"global_auditor";
    ElGamalKeypair::new_from_signer(kp, message)
        .map_err(|e| anyhow::anyhow!("Failed to create ElGamal keypair: {}", e))
}

pub fn ae_key_deterministic(kp: &dyn Signer) -> Result<AeKey> {
    let message = b"global_auditor_aes";
    AeKey::new_from_signer(kp, message)
        .map_err(|e| anyhow::anyhow!("Failed to derive AE key: {}", e))
}

/// Derive confidential keys for a given mint and owner.
///
/// Uses the ATA address as the seed for deterministic key derivation.
pub fn confidential_keys_for_mint(
    owner: Arc<dyn Signer + Send + Sync>,
    mint: &Pubkey,
) -> Result<ConfidentialKeys> {
    let ata =
        get_associated_token_address_with_program_id(&owner.pubkey(), mint, &spl_token_2022::id());
    ConfidentialKeys::from_signer(owner.as_ref(), &ata.to_bytes())
        .map_err(|e| anyhow::anyhow!("Failed to derive confidential keys: {}", e))
}
