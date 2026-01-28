use crate::{
    AppState,
    handlers::{ApiResponse, AppError},
    solana::supply::get_confidential_supply,
};
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_pubkey::Pubkey;
use std::sync::Arc;

#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplyPath {
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
}

#[serde_as]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplyResponse {
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub current_supply: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub decryptable_supply: u64,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<SupplyPath>,
) -> Result<ApiResponse<SupplyResponse>, AppError> {
    let supply = get_confidential_supply(
        state.rpc_client.clone(),
        &path.mint,
        state.elgamal_keypair.as_ref(),
        state.supply_aes_key.as_ref(),
    )
    .await
    .map_err(|err| AppError::internal_server_error(err))?;

    Ok(ApiResponse::new(SupplyResponse {
        mint: path.mint,
        current_supply: supply.current_supply,
        decryptable_supply: supply.decryptable_supply,
    }))
}
