use crate::AppState;
use crate::db;
use crate::handlers::{ApiResponse, AppError};
use crate::solana;
use crate::solana::transfer::with_split_proofs;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::Signature;
use solana_pubkey::Pubkey;
use spl_token_2022::{extension::StateWithExtensionsOwned, state::Mint};
use std::sync::Arc;
use tokio::task;

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
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub signatures: Vec<Signature>,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTransferRequest>,
) -> Result<ApiResponse<TransferResponse>, AppError> {
    if !db::wallet_exists(&state.db, &payload.source).await? {
        return Err(AppError::not_found(anyhow::anyhow!("Wallet not found")));
    }

    if !solana::tokens::is_confidential_mint_enabled(state.rpc_client.clone(), &payload.mint)
        .await?
    {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Mint is not confidential"
        )));
    }

    let requires_recipient_confidential_extension = {
        let (_, maybe_recipient_ata_account) = solana::tokens::get_maybe_ata(
            state.rpc_client.clone(),
            &payload.recipient,
            &payload.mint,
        )
        .await?;
        if maybe_recipient_ata_account.is_none() {
            return Err(AppError::bad_request(anyhow::anyhow!(
                "Recipient confidential token account for {:?} not found",
                payload.recipient
            )));
        }
        solana::tokens::ata_has_confidential_transfer_extension(
            maybe_recipient_ata_account,
            &payload.recipient,
            &payload.mint,
        )?
    };

    if requires_recipient_confidential_extension {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Recipient confidential token account for {:?} is not configured",
            payload.recipient
        )));
    }

    let mint_account = state
        .rpc_client
        .get_account(&payload.mint)
        .await
        .map_err(|e| {
            AppError::internal_server_error(anyhow::anyhow!("Mint account not found: {:?}", e))
        })?;
    let mint_state = StateWithExtensionsOwned::<Mint>::unpack(mint_account.data).map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to unpack mint account: {:?}", e))
    })?;
    let mint_decimals = mint_state.base.decimals;

    let Some(wallet) = db::get_wallet_by_pubkey(&state.db, &payload.source).await? else {
        return Err(AppError::not_found(anyhow::anyhow!("Wallet not found")));
    };

    // TODO: how to handle when the user needs to sign on the client?
    let sender_kp = Arc::new(wallet.keypair);
    let recipient = payload.recipient;
    let mint = payload.mint;
    let amount = payload.amount;

    // TODO: we can remove this `spawn_blocking` if we remove `ProgramRpcClientSendTransaction`
    let transfer_signatures = task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(with_split_proofs(
            state.rpc_client.clone(),
            sender_kp,
            &recipient,
            amount,
            &mint,
            mint_decimals,
        ))
    })
    .await
    .map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to join transfer task: {:?}", e))
    })?
    .map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to create transfer: {:?}", e))
    })?;

    Ok(ApiResponse::new(TransferResponse {
        signatures: transfer_signatures,
    }))
}
