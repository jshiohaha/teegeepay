use crate::{
    AppState,
    handlers::{ApiResponse, AppError},
    solana::supply::get_confidential_supply,
};
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_pubkey::Pubkey;
use spl_token_2022::extension::ExtensionType;
use std::sync::Arc;
use tracing::error;

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
    let conffidential_features = crate::solana::tokens::get_enabled_confidential_features(
        state.rpc_client.clone(),
        &path.mint,
    )
    .await
    .map_err(|e| {
        error!("failed to get enabled confidential features: {}", e);
        AppError::internal_server_error(anyhow::anyhow!(
            "Failed to get enabled confidential features: {}",
            e
        ))
    })?;

    if !conffidential_features.contains(&ExtensionType::ConfidentialMintBurn) {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Mint does not support confidential mint/burn extension",
        )));
    }

    let supply = get_confidential_supply(
        state.rpc_client.clone(),
        &path.mint,
        state.elgamal_keypair.as_ref(),
        state.supply_aes_key.as_ref(),
    )
    .await
    .map_err(AppError::internal_server_error)?;

    Ok(ApiResponse::new(SupplyResponse {
        mint: path.mint,
        current_supply: supply.current_supply,
        decryptable_supply: supply.decryptable_supply,
    }))
}
