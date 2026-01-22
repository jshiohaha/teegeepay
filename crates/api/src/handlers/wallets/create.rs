use crate::AppState;
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

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWalletRequest {
    /// External user ID
    pub id: String,
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
    Json(payload): Json<CreateWalletRequest>,
) -> Result<ApiResponse<CreateWalletResponse>, AppError> {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();

    info!("keypair: {:?}", keypair.to_base58_string());
    info!("pubkey: {:?}", pubkey);

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

    db::create_user_and_wallet(state.db.clone(), &payload.id, &pubkey, &keypair)
        .await
        .map_err(|e| {
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to create user and wallet: {}",
                e
            ))
        })?;

    Ok(ApiResponse::new(CreateWalletResponse { pubkey }))
}
