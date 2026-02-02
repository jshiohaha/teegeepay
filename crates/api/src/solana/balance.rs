//! Confidential token balance operations for SPL Token-2022.
//!
//! Provides functions to decrypt and query confidential balances on token
//! accounts that have the confidential transfer extension enabled, as well
//! as applying pending balance credits to the available balance.
//!
//! Two variants are exposed for each operation:
//! - `*_with_keys` — accepts pre-derived [`ConfidentialKeys`], suitable for
//!   browser wallet flows where keys are derived from a user signature.
//! - convenience wrappers — derive keys automatically from a [`Signer`].

use crate::solana::confidential_keys::ConfidentialKeys;
use crate::solana::{GeneratedInstructions, utils::confidential_keys_for_mint};
use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::{
    error::TokenError,
    extension::{
        BaseStateWithExtensions, StateWithExtensionsOwned,
        confidential_transfer::account_info::ApplyPendingBalanceAccountInfo,
    },
    solana_zk_sdk::encryption::auth_encryption::AeCiphertext,
    state::Account,
};
use spl_token_2022_interface::extension::confidential_transfer::{
    ConfidentialTransferAccount, PENDING_BALANCE_LO_BIT_LENGTH, instruction,
};
use std::sync::Arc;

/// Get confidential balances using pre-derived keys.
///
/// # Arguments
/// * `rpc_client` - RPC client
/// * `token_account_owner_pubkey` - Public key of the token account owner
/// * `mint` - Mint address
/// * `confidential_keys` - Pre-derived ElGamal and AE keypairs
///
/// # Browser Wallet Flow
/// ```ignore
/// let ata = get_associated_token_address_with_program_id(&wallet_pubkey, &mint, &spl_token_2022::id());
/// let keys = ConfidentialKeys::from_signature(wallet_pubkey, signature, &ata.to_bytes())?;
/// get_confidential_balances_with_keys(rpc_client, &wallet_pubkey, &mint, keys).await?;
/// ```
pub async fn get_confidential_balances_with_keys(
    rpc_client: Arc<RpcClient>,
    token_account_owner_pubkey: &Pubkey,
    mint: &Pubkey,
    confidential_keys: &ConfidentialKeys,
) -> Result<(u64, u64)> {
    let token_account_pubkey = get_associated_token_address_with_program_id(
        token_account_owner_pubkey,
        mint,
        &spl_token_2022::id(),
    );

    let token_account_info = rpc_client.get_account(&token_account_pubkey).await?;
    let token_account = StateWithExtensionsOwned::<Account>::unpack(token_account_info.data)?;
    let extension_data = token_account.get_extension::<ConfidentialTransferAccount>()?;

    let pending_balance_lo = extension_data
        .pending_balance_lo
        .try_into()
        .map_err(|_| anyhow::anyhow!("Failed to parse pending balance low bits"))?;
    let pending_balance_hi = extension_data
        .pending_balance_hi
        .try_into()
        .map_err(|_| anyhow::anyhow!("Failed to parse pending balance high bits"))?;
    let pending_balance_lo = confidential_keys
        .elgamal_keypair
        .secret()
        .decrypt_u32(&pending_balance_lo)
        .ok_or(anyhow::anyhow!(
            "Failed to decrypt pending balance low bits"
        ))?;
    let pending_balance_hi = confidential_keys
        .elgamal_keypair
        .secret()
        .decrypt_u32(&pending_balance_hi)
        .ok_or(anyhow::anyhow!(
            "Failed to decrypt pending balance high bits"
        ))?;
    let pending_balance = pending_balance_hi
        .checked_shl(PENDING_BALANCE_LO_BIT_LENGTH)
        .and_then(|hi| hi.checked_add(pending_balance_lo))
        .ok_or(anyhow::anyhow!("Failed to combine pending balance parts"))?;

    let decryptable_available_balance: AeCiphertext =
        extension_data.decryptable_available_balance.try_into()?;
    let available_balance = confidential_keys
        .ae_key
        .decrypt(&decryptable_available_balance)
        .ok_or(anyhow::anyhow!("Failed to decrypt available balance"))?;

    Ok((pending_balance, available_balance))
}

/// Get confidential balances - convenience wrapper that derives keys from a signer.
pub async fn get_confidential_balances(
    rpc_client: Arc<RpcClient>,
    token_account_owner: Arc<dyn Signer + Send + Sync + 'static>,
    mint: &Pubkey,
) -> Result<(u64, u64)> {
    let confidential_keys = confidential_keys_for_mint(token_account_owner.clone(), mint)?;
    get_confidential_balances_with_keys(
        rpc_client,
        &token_account_owner.pubkey(),
        mint,
        &confidential_keys,
    )
    .await
}

/// Apply pending balance to available balance using pre-derived keys.
///
/// # Arguments
/// * `rpc_client` - RPC client
/// * `ata_authority_pubkey` - Public key of the token account owner
/// * `mint` - Mint address
/// * `confidential_keys` - Pre-derived ElGamal and AE keypairs
pub async fn apply_pending_balance_with_keys(
    rpc_client: Arc<RpcClient>,
    ata_authority_pubkey: &Pubkey,
    mint: &Pubkey,
    confidential_keys: &ConfidentialKeys,
) -> Result<GeneratedInstructions> {
    let token_account_pubkey = get_associated_token_address_with_program_id(
        ata_authority_pubkey,
        mint,
        &spl_token_2022::id(),
    );

    let token_account_info = rpc_client.get_account(&token_account_pubkey).await?;
    let token_account = StateWithExtensionsOwned::<Account>::unpack(token_account_info.data)?;

    let confidential_transfer_account =
        token_account.get_extension::<ConfidentialTransferAccount>()?;
    let apply_pending_balance_account_info =
        ApplyPendingBalanceAccountInfo::new(confidential_transfer_account);
    let expected_pending_balance_credit_counter =
        apply_pending_balance_account_info.pending_balance_credit_counter();
    let new_decryptable_available_balance = apply_pending_balance_account_info
        .new_decryptable_available_balance(
            confidential_keys.elgamal_keypair.secret(),
            &confidential_keys.ae_key,
        )
        .map_err(|_| TokenError::AccountDecryption)?;
    let apply_pending_balance_instruction = instruction::apply_pending_balance(
        &spl_token_2022::id(),
        &token_account_pubkey,                     // Token account
        expected_pending_balance_credit_counter, // Expected number of times the pending balance has been credited
        &new_decryptable_available_balance.into(), // Cipher text of the new decryptable available balance
        ata_authority_pubkey,                      // Token account owner
        &[ata_authority_pubkey],                   // Additional signers
    )?;

    Ok(GeneratedInstructions {
        instructions: vec![apply_pending_balance_instruction],
        additional_signers: vec![],
    })
}

/// Apply pending balance - convenience wrapper that derives keys from a signer.
pub async fn apply_pending_balance(
    rpc_client: Arc<RpcClient>,
    ata_authority: Arc<dyn Signer + Send + Sync>,
    _fee_payer: Arc<dyn Signer + Send + Sync>,
    mint: &Pubkey,
    _decimals: u8,
) -> Result<GeneratedInstructions> {
    let confidential_keys = confidential_keys_for_mint(ata_authority.clone(), mint)?;
    let mut result = apply_pending_balance_with_keys(
        rpc_client,
        &ata_authority.pubkey(),
        mint,
        &confidential_keys,
    )
    .await?;

    result.additional_signers.push(ata_authority);

    Ok(result)
}
