use crate::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use crate::solana::airdrop::request_and_confirm;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::Arc;
use tracing::info;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWalletRequest {
    #[serde_as(as = "Option<serde_with::Bytes>")]
    pub bytes: Option<Vec<u8>>,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWalletResponse {
    #[serde_as(as = "DisplayFromStr")]
    pub pubkey: Pubkey,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<CreateWalletRequest>,
) -> Result<ApiResponse<CreateWalletResponse>, AppError> {
    let keypair = if let Some(bytes) = payload.bytes {
        Keypair::try_from(bytes.as_slice())
    } else {
        Ok(Keypair::new())
    }
    .map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("failed to create keypair: {}", e))
    })?;

    let pubkey = keypair.pubkey();

    db::create_wallet_for_telegram_user(
        &state.db,
        auth_user.telegram_user_id,
        &pubkey,
        &keypair,
    )
    .await
    .map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to create wallet: {}", e))
    })?;

    let signature = request_and_confirm(state.rpc_client.clone(), &pubkey, 1 * 10_u64.pow(9))
        .await
        .map_err(|e| {
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to request and confirm airdrop: {}",
                e
            ))
        })?;

    info!(
        "Requested airdrop for pubkey={:?} with signature={:?}",
        pubkey, signature
    );

    Ok(ApiResponse::new(CreateWalletResponse { pubkey }))
}
