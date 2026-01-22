use std::sync::Arc;

use anyhow::Result;
use solana_keypair::{Keypair, Signature};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::solana_zk_sdk::encryption::elgamal::ElGamalKeypair;

use crate::solana::signature_signer::{ConfidentialKeys, SignatureKeyDerivation};

pub fn create_keypair() -> Keypair {
    Keypair::new()
}

pub fn create_keypair_elgamal() -> ElGamalKeypair {
    ElGamalKeypair::new_rand()
}

/// Derive confidential keys for a given mint and owner.
///
/// # Arguments
/// * `owner` - The owner of the token account
/// * `mint` - The mint address
///
/// # Returns
/// * `ConfidentialKeys` - The confidential keys for the given mint and owner
///
/// # Example
/// ```ignore
/// let confidential_keys = confidential_keys_for_mint(owner, mint)?;
/// ```
///
/// # Browser Wallet Flow
/// ```ignore
/// let ata = get_associated_token_address_with_program_id(&owner.pubkey(), &mint, &spl_token_2022::id());
/// let signature = owner.sign_message(&ata.to_bytes());
/// let confidential_keys = confidential_keys_for_mint(owner, mint)?;
/// ```
pub fn confidential_keys_for_mint(
    owner: Arc<dyn Signer + Send + Sync>,
    mint: &Pubkey,
) -> Result<ConfidentialKeys> {
    let owner_pubkey = owner.pubkey();
    let ata =
        get_associated_token_address_with_program_id(&owner_pubkey, &mint, &spl_token_2022::id());

    let signature = owner.sign_message(&ata.to_bytes());
    let signature_bytes: [u8; 64] = signature.into();

    let key_derivation = SignatureKeyDerivation::new(owner_pubkey, signature_bytes.to_vec());

    key_derivation
        .derive_keys(&ata.to_bytes())
        .map_err(|e| anyhow::anyhow!("Failed to derive confidential keys: {}", e))
}

#[allow(dead_code)]
pub fn confidential_keys_from_signature(
    owner: &Pubkey,
    mint: &Pubkey,
    signature: Signature,
) -> Result<ConfidentialKeys> {
    let ata = get_associated_token_address_with_program_id(&owner, &mint, &spl_token_2022::id());
    let signature_bytes: [u8; 64] = signature.into();

    SignatureKeyDerivation::new(*owner, signature_bytes.to_vec()).derive_keys(&ata.to_bytes())
}
