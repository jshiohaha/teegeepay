use crate::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use crate::solana::airdrop::request_and_confirm;
use axum::body::Bytes;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::Arc;
use tracing::{info, error};

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
    info!("[CREATE_WALLET] ========== CREATE WALLET HANDLER ==========");
    info!("[CREATE_WALLET] telegram_user_id: {}, username: {:?}", 
        auth_user.telegram_user_id, auth_user.username);
    info!("[CREATE_WALLET] payload length: {}", payload.len());
    
    let payload = if payload.is_empty() {
        info!("[CREATE_WALLET] No payload, will generate new keypair");
        None
    } else {
        info!("[CREATE_WALLET] Parsing payload");
        Some(
            serde_json::from_slice::<CreateWalletRequest>(&payload)
                .map_err(|e| {
                    error!("[CREATE_WALLET] Failed to parse payload: {}", e);
                    AppError::bad_request(anyhow::anyhow!("Invalid JSON body: {}", e))
                })?,
        )
    };

    let keypair = if let Some(payload) = payload
        && payload.bytes.is_some()
    {
        info!("[CREATE_WALLET] Using provided keypair bytes");
        let bytes = payload.bytes.unwrap_or_default();
        Keypair::try_from(bytes.as_slice()).map_err(|e| {
            error!("[CREATE_WALLET] Failed to create keypair from bytes: {}", e);
            AppError::internal_server_error(anyhow::anyhow!("failed to create keypair: {}", e))
        })?
    } else {
        info!("[CREATE_WALLET] Generating new keypair");
        Keypair::new()
    };

    let pubkey = keypair.pubkey();
    info!("[CREATE_WALLET] Pubkey: {}", pubkey);

    info!("[CREATE_WALLET] Saving wallet to database for telegram_user_id: {}", auth_user.telegram_user_id);
    let wallet_id = db::create_wallet_for_telegram_user(&state.db, auth_user.telegram_user_id, &pubkey, &keypair)
        .await
        .map_err(|e| {
            error!("[CREATE_WALLET] Failed to save wallet to DB: {}", e);
            AppError::internal_server_error(anyhow::anyhow!("Failed to create wallet: {}", e))
        })?;
    info!("[CREATE_WALLET] Wallet saved with id: {}", wallet_id);

    info!("[CREATE_WALLET] Requesting airdrop for pubkey: {}", pubkey);
    let signature = request_and_confirm(state.rpc_client.clone(), &pubkey, 1 * 10_u64.pow(9))
        .await
        .map_err(|e| {
            error!("[CREATE_WALLET] Airdrop failed: {}", e);
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to request and confirm airdrop: {}",
                e
            ))
        })?;

    info!("[CREATE_WALLET] Airdrop successful, signature: {:?}", signature);
    info!("[CREATE_WALLET] ========== CREATE WALLET SUCCESS ==========");

    Ok(ApiResponse::new(CreateWalletResponse { pubkey }))
}
