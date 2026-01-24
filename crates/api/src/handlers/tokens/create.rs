use crate::solana::create::create_mint;
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
use tracing::info;

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
    let mint_keypair = payload
        .mint_keypair
        .map(|kp| Keypair::from_base58_string(&kp))
        .unwrap_or(Keypair::new());
    let mint_keypair = Arc::new(mint_keypair);
    let mint_pubkey = mint_keypair.pubkey();

    info!("New mint: {:?}", mint_pubkey);

    let global_authority = state.global_authority.clone();
    let create_mint_instructions = create_mint(
        state.rpc_client.clone(),
        global_authority.clone(),
        global_authority.clone(),
        state.elgamal_keypair.clone(),
        Some(mint_keypair.clone()),
        Some(payload.decimals),
    )
    .await?;

    info!("Build create transaction for mint={:?}", mint_pubkey);

    let transaction = build_transaction(
        state.rpc_client.clone(),
        None,
        create_mint_instructions.instructions,
        global_authority.clone(),
        create_mint_instructions.additional_signers,
    )
    .await?;

    info!("Sending create transaction for mint={:?}", mint_pubkey);

    let transaction_signature = state
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send and confirm transaction: {}", e))?;

    info!(
        "Created create transaction for mint={:?} with signature={:?}",
        mint_pubkey, transaction_signature
    );

    Ok(ApiResponse::new(CreateTokenResponse {
        address: mint_pubkey,
        signature: transaction_signature,
    }))
}
