use crate::solana::create::{ConfidentialMintBurnParams, CreateMintParams, create_mint};
use crate::solana::transaction::build_transaction;
use crate::{
    AppState,
    handlers::{ApiResponse, AppError},
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::{Keypair, Signature};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::Arc;
use tracing::error;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTokenRequest {
    /// Name of the new token
    #[serde_as(as = "DisplayFromStr")]
    pub name: String,
    /// Symbol of the new token
    #[serde_as(as = "DisplayFromStr")]
    pub symbol: String,
    /// URI for additional metadata
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub uri: Option<String>,
    /// Decimals for the new token
    pub decimals: u8,
    /// Optional keypair of the new token
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub mint_keypair: Option<String>,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTokenResponse {
    #[serde_as(as = "DisplayFromStr")]
    pub address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub signature: Signature,
}

// handler is at POST /tokens
pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTokenRequest>,
) -> Result<ApiResponse<CreateTokenResponse>, AppError> {
    let CreateTokenRequest {
        name,
        symbol,
        uri,
        decimals,
        mint_keypair: mint_keypair_b58,
    } = payload;

    let mint_keypair = mint_keypair_b58
        .map(|kp| Keypair::from_base58_string(&kp))
        .unwrap_or(Keypair::new());
    let mint_keypair = Arc::new(mint_keypair);
    let mint_pubkey = mint_keypair.pubkey();

    let global_authority = state.global_authority.clone();
    let create_mint_instructions = create_mint(CreateMintParams {
        rpc_client: state.rpc_client.clone(),
        fee_payer: global_authority.clone(),
        authority: global_authority.clone(),
        auditor_elgamal_keypair: state.elgamal_keypair.clone(),
        mint: Some(mint_keypair.clone()),
        decimals: Some(decimals),
        name,
        symbol,
        metadata_uri: uri,
        confidential_mint_burn: Some(ConfidentialMintBurnParams {
            supply_aes_key: state.supply_aes_key.clone(),
        }),
    })
    .await
    .map_err(|e| {
        error!("failed to create mint: {}", e);
        AppError::internal_server_error(anyhow::anyhow!("Failed to create mint: {}", e))
    })?;

    let transaction = build_transaction(
        state.rpc_client.clone(),
        None,
        create_mint_instructions.instructions,
        global_authority.clone(),
        create_mint_instructions.additional_signers,
    )
    .await
    .map_err(|e| {
        error!("failed to build transaction: {}", e);
        AppError::internal_server_error(anyhow::anyhow!("Failed to build transaction: {}", e))
    })?;

    let transaction_signature = state
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .map_err(|e| {
            error!("failed to send and confirm transaction: {}", e);
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to send and confirm transaction: {}",
                e
            ))
        })?;

    Ok(ApiResponse::new(CreateTokenResponse {
        address: mint_pubkey,
        signature: transaction_signature,
    }))
}
