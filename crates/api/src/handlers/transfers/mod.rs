use crate::handlers::AppError;
use crate::handlers::wallets::deposit::TransactionResult;
use crate::solana::transfer;
use crate::{AppState, db, solana};
use axum::{Router, routing::post};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_keypair::{Keypair, Signature};
use solana_pubkey::Pubkey;
use spl_token_2022::extension::ExtensionType;
use spl_token_2022::{extension::StateWithExtensionsOwned, state::Mint};
use std::sync::Arc;
use tokio::task;

pub mod create;
pub mod telegram;

pub const TRANSFER_TRANSACTION_LABELS: [&str; 5] = [
    "Create Proof Accounts",
    "Verify Proof Accounts: Range",
    "Verify Proof Accounts: Equality, Ciphertext",
    "Transfer",
    "Close Proof Accounts",
];

/// nested within /transfers prefix
pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(create::handler))
        .route("/telegram", post(telegram::handler))
        .with_state(state)
}

pub async fn validate_sender_wallet(
    state: &AppState,
    source: &Pubkey,
    telegram_user_id: i64,
) -> Result<crate::models::Wallet, AppError> {
    let Some(wallet) = db::get_user_wallet_by_pubkey(&state.db, source, telegram_user_id).await?
    else {
        return Err(AppError::not_found(anyhow::anyhow!(
            "Wallet not found or not authorized"
        )));
    };

    if wallet.pubkey != *source {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Wallet address does not match provided address"
        )));
    }

    Ok(wallet)
}

pub async fn validate_confidential_mint(state: &AppState, mint: &Pubkey) -> Result<(), AppError> {
    let enabled_confidential_features =
        solana::tokens::get_enabled_confidential_features(state.rpc_client.clone(), mint).await?;
    if !enabled_confidential_features.contains(&ExtensionType::ConfidentialTransferMint) {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Mint is not confidential"
        )));
    }

    Ok(())
}

pub async fn get_mint_decimals(state: &AppState, mint: &Pubkey) -> Result<u8, AppError> {
    let mint_account = state.rpc_client.get_account(mint).await.map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Mint account not found: {:?}", e))
    })?;
    let mint_state = StateWithExtensionsOwned::<Mint>::unpack(mint_account.data).map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to unpack mint account: {:?}", e))
    })?;
    Ok(mint_state.base.decimals)
}

pub async fn execute_transfer(
    rpc_client: Arc<RpcClient>,
    sender_kp: Arc<Keypair>,
    recipient_pubkey: &Pubkey,
    amount: u64,
    mint: Pubkey,
    mint_decimals: u8,
) -> Result<Vec<Signature>, AppError> {
    let recipient_pubkey = *recipient_pubkey;

    task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(transfer::invoke_confidential_transfer(
            rpc_client,
            sender_kp,
            &recipient_pubkey,
            amount,
            &mint,
            mint_decimals,
        ))
    })
    .await
    .map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to join transfer task: {:?}", e))
    })?
    .map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to create transfer: {:?}", e))
    })
}

pub fn format_transfer_results(transfer_signatures: &[Signature]) -> Vec<TransactionResult> {
    TRANSFER_TRANSACTION_LABELS
        .iter()
        .zip(transfer_signatures.iter())
        .map(|(label, signature)| TransactionResult {
            label: label.to_string(),
            signature: *signature,
        })
        .collect()
}
