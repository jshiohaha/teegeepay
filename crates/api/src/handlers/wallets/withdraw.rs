use crate::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use crate::solana;
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
use tokio::task;
use tracing::info;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawTokensRequest {
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
pub struct WithdrawTokensResponse {
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub signatures: Vec<Signature>,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawTokensPath {
    #[serde_as(as = "DisplayFromStr")]
    pub address: Pubkey,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(path): Path<WithdrawTokensPath>,
    Json(payload): Json<WithdrawTokensRequest>,
) -> Result<ApiResponse<WithdrawTokensResponse>, AppError> {
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

    // TODO: do this conditionally?
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
    info!(
        "Withdraw [Apply Pending Balance] with signature={:?}",
        apply_signature
    );

    // TODO: we can remove this after removing `ProgramRpcClientSendTransaction`
    let withdraw_signatures: Vec<Signature> = task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(solana::withdraw::withdraw_tokens(
            state.rpc_client.clone(),
            owner_kp.clone(),
            payload.amount,
            &payload.mint,
            payload.decimals,
        ))
    })
    .await
    .with_context(|| anyhow::anyhow!("Failed to join withdraw task"))
    .map_err(AppError::from)?
    .with_context(|| anyhow::anyhow!("Failed to create withdraw"))
    .map_err(AppError::from)?;

    Ok(ApiResponse::new(WithdrawTokensResponse {
        signatures: withdraw_signatures,
    }))
}
