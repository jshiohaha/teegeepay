use crate::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_pubkey::Pubkey;
use std::sync::Arc;
use tracing::{info, error};

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWalletsResponse {
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub pubkeys: Vec<Pubkey>,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<ApiResponse<ListWalletsResponse>, AppError> {
    info!("[WALLETS] list handler called for telegram_user_id: {}", auth_user.telegram_user_id);
    
    let pubkeys = db::get_wallets_for_telegram_user(&state.db, auth_user.telegram_user_id)
        .await
        .map_err(|e| {
            error!("[WALLETS] Failed to get wallets for user {}: {}", auth_user.telegram_user_id, e);
            AppError::internal_server_error(anyhow::anyhow!("Failed to get wallets: {}", e))
        })?;

    info!("[WALLETS] Found {} wallets for user {}", pubkeys.len(), auth_user.telegram_user_id);
    Ok(ApiResponse::new(ListWalletsResponse { pubkeys }))
}
