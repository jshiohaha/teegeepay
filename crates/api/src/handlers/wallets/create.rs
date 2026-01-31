use crate::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use crate::kms;
use crate::solana::airdrop::request_airdrop_and_confirm;
use axum::body::Bytes;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_pubkey::Pubkey;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

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
    payload: Bytes,
) -> Result<ApiResponse<CreateWalletResponse>, AppError> {
    let payload = if payload.is_empty() {
        None
    } else {
        Some(
            serde_json::from_slice::<CreateWalletRequest>(&payload).map_err(|e| {
                error!("failed to parse payload: {}", e);
                AppError::bad_request(anyhow::anyhow!("Invalid JSON body: {}", e))
            })?,
        )
    };

    if let Some(payload) = payload {
        if payload.bytes.is_some() {
            error!("received raw keypair bytes but KMS wallets cannot import keys");
            return Err(AppError::bad_request(anyhow::anyhow!(
                "Importing existing keypairs is not supported when using AWS KMS"
            )));
        }
    }

    let kms_alias = format!("cypherpay/wallet/{}", Uuid::new_v4());
    info!(
        "creating AWS KMS key with alias: {} for user_id: {}",
        kms_alias, auth_user.telegram_user_id
    );
    let kms_key_id = kms::create_kms_ed25519_key(&state.kms_client, &kms_alias)
        .await
        .map_err(|e| {
            error!("failed to create KMS key: {}", e);
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to create KMS key for wallet: {}",
                e
            ))
        })?;
    let pubkey = kms::solana_pubkey_from_kms(&state.kms_client, &kms_key_id)
        .await
        .map_err(|e| {
            error!("failed to derive Solana pubkey from KMS: {}", e);
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to derive wallet pubkey from KMS key: {}",
                e
            ))
        })?;

    db::create_wallet_for_telegram_user(
        &state.db,
        auth_user.telegram_user_id,
        &pubkey,
        &kms_key_id,
    )
    .await
    .map_err(|e| {
        error!("failed to save wallet to DB: {}", e);
        AppError::internal_server_error(anyhow::anyhow!("Failed to create wallet: {}", e))
    })?;

    request_airdrop_and_confirm(state.rpc_client.clone(), &pubkey, 1 * 10_u64.pow(9))
        .await
        .map_err(|e| anyhow::anyhow!("failed to fund wallet: {}", e))?;

    Ok(ApiResponse::new(CreateWalletResponse { pubkey }))
}
