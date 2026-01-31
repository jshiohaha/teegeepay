use crate::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use crate::solana::airdrop::request_airdrop_and_confirm;
use axum::body::Bytes;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::Arc;
use tracing::{error, info};

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
        info!("no payload, generate new keypair");
        None
    } else {
        Some(
            serde_json::from_slice::<CreateWalletRequest>(&payload).map_err(|e| {
                error!("failed to parse payload: {}", e);
                AppError::bad_request(anyhow::anyhow!("Invalid JSON body: {}", e))
            })?,
        )
    };

    let keypair = if let Some(payload) = payload
        && payload.bytes.is_some()
    {
        let bytes = payload.bytes.unwrap_or_default();
        Keypair::try_from(bytes.as_slice()).map_err(|e| {
            error!("failed to create keypair from bytes: {}", e);
            AppError::bad_request(anyhow::anyhow!("failed to create keypair: {}", e))
        })?
    } else {
        info!("generating new keypair");
        Keypair::new()
    };

    let pubkey = keypair.pubkey();
    request_airdrop_and_confirm(state.rpc_client.clone(), &pubkey, 1 * 10_u64.pow(9))
        .await
        .map_err(AppError::from)?;

    db::create_wallet_for_telegram_user(&state.db, auth_user.telegram_user_id, &pubkey, &keypair)
        .await
        .map_err(AppError::from)?;

    Ok(ApiResponse::new(CreateWalletResponse { pubkey }))
}
