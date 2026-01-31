use crate::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use crate::solana::airdrop::request_and_confirm;
use axum::extract::Path;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::Signature;
use solana_pubkey::Pubkey;
use std::sync::Arc;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirdropRequest {
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub amount: Option<u64>,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirdropPath {
    #[serde_as(as = "DisplayFromStr")]
    pub address: Pubkey,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirdropResponse {
    #[serde_as(as = "DisplayFromStr")]
    pub signature: Signature,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: u64,
}

// POST /wallets/{address}/airdrop
pub async fn handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(path): Path<AirdropPath>,
    Json(payload): Json<AirdropRequest>,
) -> Result<ApiResponse<AirdropResponse>, AppError> {
    let Some(wallet) =
        db::get_user_wallet_by_pubkey(&state.db, &path.address, auth_user.telegram_user_id).await?
    else {
        return Err(AppError::not_found(anyhow::anyhow!(
            "Wallet not found or not authorized"
        )));
    };

    let requester = wallet.pubkey;
    let amount = payload.amount.unwrap_or(1) * 10_u64.pow(9);
    let signature = request_and_confirm(state.rpc_client.clone(), &requester, amount)
        .await
        .map_err(|e| {
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to request and confirm airdrop: {}",
                e
            ))
        })?;

    Ok(ApiResponse::new(AirdropResponse { signature, amount }))
}
