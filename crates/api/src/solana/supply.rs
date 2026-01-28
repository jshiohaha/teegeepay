use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use spl_token_2022::{
    extension::{
        BaseStateWithExtensions, StateWithExtensionsOwned,
        confidential_mint_burn::{ConfidentialMintBurn, account_info::SupplyAccountInfo},
    },
    solana_zk_sdk::encryption::{
        auth_encryption::{AeCiphertext, AeKey},
        elgamal::ElGamalKeypair,
    },
    state::Mint,
};
use std::sync::Arc;

/// Decrypted supply information taken from the `ConfidentialMintBurn` extension.
#[derive(Debug, Clone)]
pub struct ConfidentialSupply {
    /// The audited supply derived from the ElGamal ciphertext and AE key.
    pub current_supply: u64,
    /// The raw decryptable supply stored on-chain.
    pub decryptable_supply: u64,
}

/// Decrypt the current supply for a confidential mint using the supply's keys.
pub async fn get_confidential_supply(
    rpc_client: Arc<RpcClient>,
    mint: &Pubkey,
    supply_elgamal_keypair: &ElGamalKeypair,
    supply_aes_key: &AeKey,
) -> Result<ConfidentialSupply> {
    let mint_account = rpc_client.get_account(mint).await?;
    let mint_state = StateWithExtensionsOwned::<Mint>::unpack(mint_account.data)?;
    let extension = mint_state
        .get_extension::<ConfidentialMintBurn>()
        .map_err(|_| anyhow::anyhow!("Mint does not support confidential mint/burn extension"))?;

    let supply_info = SupplyAccountInfo::new(extension);

    let decryptable_supply_ciphertext: AeCiphertext = supply_info
        .decryptable_supply
        .try_into()
        .map_err(|_| anyhow::anyhow!("Failed to parse decryptable supply ciphertext"))?;
    let decryptable_supply = supply_aes_key
        .decrypt(&decryptable_supply_ciphertext)
        .ok_or(anyhow::anyhow!("Failed to decrypt decryptable supply"))?;

    let current_supply = supply_info
        .decrypted_current_supply(supply_aes_key, supply_elgamal_keypair)
        .map_err(|e| anyhow::anyhow!("Failed to decrypt current supply: {:?}", e))?;

    Ok(ConfidentialSupply {
        current_supply,
        decryptable_supply,
    })
}
