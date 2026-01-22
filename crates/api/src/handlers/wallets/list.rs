use crate::AppState;
use crate::db;
use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_pubkey::Pubkey;
use std::sync::Arc;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWalletsResponse {
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub pubkeys: Vec<Pubkey>,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
) -> Result<ApiResponse<ListWalletsResponse>, AppError> {
    let pubkeys = db::get_all_wallets(&state.db).await.map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to get all wallets: {}", e))
    })?;

    Ok(ApiResponse::new(ListWalletsResponse { pubkeys }))
}
