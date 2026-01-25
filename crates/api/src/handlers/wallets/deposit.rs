use crate::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use crate::partial_sign::PartialSign;
use crate::solana::balance::apply_pending_balance_with_keys;
use crate::solana::transaction::build_transaction;
use crate::solana::utils::confidential_keys_for_mint;
use anyhow::Context;
use axum::extract::Path;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::Signature;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::Arc;
use tracing::info;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositTokensRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: u64,
    // TODO: migrate to rpc check?
    pub decimals: u8,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResult {
    pub label: String,
    #[serde_as(as = "DisplayFromStr")]
    pub signature: Signature,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositTokensResponse {
    pub transactions: Vec<TransactionResult>,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositTokensPath {
    #[serde_as(as = "DisplayFromStr")]
    pub address: Pubkey,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(path): Path<DepositTokensPath>,
    Json(payload): Json<DepositTokensRequest>,
) -> Result<ApiResponse<DepositTokensResponse>, AppError> {
    let address = path.address;
    let Some(wallet) =
        db::get_user_wallet_by_pubkey(&state.db, &path.address, auth_user.telegram_user_id).await?
    else {
        return Err(AppError::not_found(anyhow::anyhow!(
            "Wallet not found or not authorized"
        )));
    };

    if wallet.pubkey != address {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Wallet address does not match provided address"
        )));
    }

    let owner_kp: Arc<dyn Signer + Send + Sync> = Arc::new(wallet.keypair);
    let confidential_keys = confidential_keys_for_mint(owner_kp.clone(), &payload.mint)?;

    let mut transactions: Vec<TransactionResult> = vec![];

    if payload.amount == 0 {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Deposit amount must be greater than 0"
        )));
    }

    let deposit_instructions = crate::solana::deposit::deposit_tokens(
        state.rpc_client.clone(),
        &owner_kp.pubkey(),
        &payload.mint,
        payload.decimals,
        payload.amount,
    )
    .await?;

    let mut tx = build_transaction(
        state.rpc_client.clone(),
        None,
        deposit_instructions.instructions,
        owner_kp.clone(),
        vec![],
    )
    .await?;

    tx.partial_sign(&owner_kp).map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!(
            "Failed to partial sign transaction: {:?}",
            e
        ))
    })?;

    let deposit_signature = state
        .rpc_client
        .clone()
        .send_and_confirm_transaction(&tx)
        .await
        .with_context(|| anyhow::anyhow!("Error sending transaction"))
        .map_err(AppError::from)?;
    transactions.push(TransactionResult {
        label: "Deposit".to_string(),
        signature: deposit_signature,
    });
    info!(
        "Transfer [Deposit Confidential Pending Balance] with signature={:?}",
        deposit_signature
    );

    let apply_instructions = apply_pending_balance_with_keys(
        state.rpc_client.clone(),
        &owner_kp.pubkey(),
        &payload.mint,
        &confidential_keys,
    )
    .await?;

    let transaction = build_transaction(
        state.rpc_client.clone(),
        None,
        apply_instructions.instructions,
        owner_kp.clone(),
        apply_instructions.additional_signers.into_iter().collect(),
    )
    .await?;

    let apply_signature = state
        .rpc_client
        .clone()
        .send_and_confirm_transaction(&transaction)
        .await
        .with_context(|| anyhow::anyhow!("Error sending transaction"))
        .map_err(AppError::from)?;
    transactions.push(TransactionResult {
        label: "Apply Pending Balance".to_string(),
        signature: apply_signature,
    });
    info!(
        "Deposit [Apply Pending Balance] with signature={:?}",
        apply_signature
    );

    Ok(ApiResponse::new(DepositTokensResponse { transactions }))
}
