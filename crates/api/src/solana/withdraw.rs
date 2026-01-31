use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_keypair::{Keypair, Signature};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::extension::{
    BaseStateWithExtensions,
    confidential_transfer::{ConfidentialTransferAccount, account_info::WithdrawAccountInfo},
};
use spl_token_client::{
    client::{ProgramRpcClient, ProgramRpcClientSendTransaction},
    token::Token,
};
use spl_token_confidential_transfer_proof_generation::withdraw::WithdrawProofData;
use std::sync::Arc;
use tracing::{info, warn};

use crate::{
    kms::KmsKeypair,
    solana::{signature_signer::ConfidentialKeys, utils::confidential_keys_for_mint},
};

/// Withdraw tokens using pre-derived confidential keys.
///
/// # Arguments
/// * `rpc_client` - RPC client
/// * `withdrawer` - Withdrawer signer (for transaction signing)
/// * `amount` - Amount to withdraw
/// * `mint` - Mint address
/// * `decimals` - Token decimals
/// * `confidential_keys` - Pre-derived ElGamal and AE keypairs
///
/// # Browser Wallet Flow
/// ```ignore
/// let ata = get_associated_token_address_with_program_id(&wallet_pubkey, &mint, &spl_token_2022::id());
/// let keys = ConfidentialKeys::from_signature(wallet_pubkey, signature, &ata.to_bytes())?;
/// withdraw_tokens_with_keys(rpc_client, withdrawer, amount, mint, decimals, keys).await?;
/// ```
pub async fn withdraw_tokens_with_keys(
    rpc_client: Arc<RpcClient>,
    withdrawer: Arc<dyn Signer + Send + Sync>,
    amount: u64,
    mint: &Pubkey,
    decimals: u8,
    confidential_keys: &ConfidentialKeys,
) -> Result<Vec<Signature>> {
    let recipient_associated_token_address = get_associated_token_address_with_program_id(
        &withdrawer.pubkey(),
        mint,
        &spl_token_2022::id(),
    );

    let token = {
        let program_client = ProgramRpcClient::new(rpc_client, ProgramRpcClientSendTransaction);

        // Create a "token" client, to use various helper functions for Token Extensions
        Token::new(
            Arc::new(program_client),
            &spl_token_2022::id(),
            mint,
            Some(decimals),
            withdrawer.clone(),
        )
    };

    // Get recipient token account data
    let token_account = token
        .get_account_info(&recipient_associated_token_address)
        .await?;

    // Unpack the ConfidentialTransferAccount extension portion of the token account data
    let extension_data = token_account.get_extension::<ConfidentialTransferAccount>()?;

    // Confidential Transfer extension information needed to construct a `Withdraw` instruction.
    let withdraw_account_info = WithdrawAccountInfo::new(extension_data);

    // Authority for the withdraw proof account (to close the account)
    let context_state_authority = withdrawer.clone();

    let equality_proof_context_state_keypair = Keypair::new();
    let equality_proof_context_state_pubkey = equality_proof_context_state_keypair.pubkey();
    let range_proof_context_state_keypair = Keypair::new();
    let range_proof_context_state_pubkey = range_proof_context_state_keypair.pubkey();

    // Create a withdraw proof data
    let WithdrawProofData {
        equality_proof_data,
        range_proof_data,
    } = withdraw_account_info.generate_proof_data(
        amount,
        &confidential_keys.elgamal_keypair,
        &confidential_keys.ae_key,
    )?;

    // Generate withdrawal proof accounts
    let context_state_authority_pubkey = context_state_authority.pubkey();
    let create_equality_proof_signer = &[&equality_proof_context_state_keypair];
    let create_range_proof_signer = &[&range_proof_context_state_keypair];

    let mut signatures = vec![];

    info!("create equality proof");
    let equality_response = token
        .confidential_transfer_create_context_state_account(
            &equality_proof_context_state_pubkey,
            &context_state_authority_pubkey,
            &equality_proof_data,
            false,
            create_equality_proof_signer,
        )
        .await?;
    if let Some(s) = get_maybe_signature(equality_response, &withdrawer.pubkey())? {
        signatures.push(s.clone());
    }

    info!("create range proof");
    let range_response = token
        .confidential_transfer_create_context_state_account(
            &range_proof_context_state_pubkey,
            &context_state_authority_pubkey,
            &range_proof_data,
            true,
            create_range_proof_signer,
        )
        .await?;
    if let Some(s) = get_maybe_signature(range_response, &withdrawer.pubkey())? {
        signatures.push(s.clone());
    }

    info!("creating withdraw");
    let withdraw_response = token
        .confidential_transfer_withdraw(
            &recipient_associated_token_address,
            &withdrawer.pubkey(),
            Some(&equality_proof_context_state_pubkey),
            Some(&range_proof_context_state_pubkey),
            amount,
            decimals,
            Some(withdraw_account_info),
            &confidential_keys.elgamal_keypair,
            &confidential_keys.ae_key,
            &[&withdrawer],
        )
        .await?;
    if let Some(s) = get_maybe_signature(withdraw_response, &withdrawer.pubkey())? {
        signatures.push(s.clone());
    }

    let close_context_state_signer = &[&context_state_authority];

    info!("closing equality proof");
    let close_equality_response = token
        .confidential_transfer_close_context_state_account(
            &equality_proof_context_state_pubkey,
            &recipient_associated_token_address,
            &context_state_authority_pubkey,
            close_context_state_signer,
        )
        .await?;
    if let Some(s) = get_maybe_signature(close_equality_response, &withdrawer.pubkey())? {
        signatures.push(s.clone());
    }

    info!("closing range proof");
    let close_range_response = token
        .confidential_transfer_close_context_state_account(
            &range_proof_context_state_pubkey,
            &recipient_associated_token_address,
            &context_state_authority_pubkey,
            close_context_state_signer,
        )
        .await?;
    if let Some(s) = get_maybe_signature(close_range_response, &withdrawer.pubkey())? {
        signatures.push(s.clone());
    }

    Ok(signatures)
}

/// Withdraw tokens - convenience wrapper that derives keys from the withdrawer signer.
pub async fn withdraw_tokens(
    rpc_client: Arc<RpcClient>,
    withdrawer: Arc<dyn Signer + Send + Sync>,
    amount: u64,
    mint: &Pubkey,
    decimals: u8,
) -> Result<Vec<Signature>> {
    let confidential_keys = confidential_keys_for_mint(withdrawer.clone(), mint)?;
    withdraw_tokens_with_keys(
        rpc_client,
        withdrawer,
        amount,
        mint,
        decimals,
        &confidential_keys,
    )
    .await
}

fn get_maybe_signature(
    response: spl_token_client::client::RpcClientResponse,
    withdrawer: &Pubkey,
) -> Result<Option<Signature>> {
    match response {
        spl_token_client::client::RpcClientResponse::Signature(s) => Ok(Some(s)),
        _ => {
            warn!(
                "Expected signature response from create equality proof from {:?}",
                withdrawer
            );
            Ok(None)
        }
    }
}
