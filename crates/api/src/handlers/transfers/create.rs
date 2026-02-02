use crate::AppState;
use crate::auth::AuthUser;
use crate::handlers::wallets::deposit::TransactionResult;
use crate::handlers::{ApiResponse, AppError};
use crate::solana;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_pubkey::Pubkey;
use std::sync::Arc;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransferRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub source: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub recipient: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: u64,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferResponse {
    pub transactions: Vec<TransactionResult>,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<CreateTransferRequest>,
) -> Result<ApiResponse<TransferResponse>, AppError> {
    let sender_wallet =
        super::validate_sender_wallet(&state, &payload.source, auth_user.telegram_user_id).await?;
    super::validate_confidential_mint(&state, &payload.mint).await?;

    let (_, maybe_recipient_ata_account) = solana::tokens::get_maybe_ata(
        state.rpc_client.clone(),
        &payload.recipient,
        &payload.mint,
    )
    .await?;
    if maybe_recipient_ata_account.is_none() {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Recipient confidential token account not found",
        )));
    }
    let requires_setup = solana::tokens::ata_has_confidential_transfer_extension(
        maybe_recipient_ata_account,
        &payload.recipient,
        &payload.mint,
    )?;
    if requires_setup {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Recipient confidential token account is not configured"
        )));
    }

    let mint_decimals = super::get_mint_decimals(&state, &payload.mint).await?;

    let transfer_signatures = super::execute_transfer(
        state.rpc_client.clone(),
        sender_wallet.keypair.clone(),
        &payload.recipient,
        payload.amount,
        payload.mint,
        mint_decimals,
    )
    .await?;

    let transactions = super::format_transfer_results(&transfer_signatures);

    Ok(ApiResponse::new(TransferResponse { transactions }))
}
