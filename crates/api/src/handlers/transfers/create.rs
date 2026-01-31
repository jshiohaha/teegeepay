use crate::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::handlers::wallets::deposit::TransactionResult;
use crate::handlers::{ApiResponse, AppError};
use crate::solana;
use crate::solana::transfer::with_split_proofs;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_pubkey::Pubkey;
use spl_token_2022::extension::ExtensionType;
use spl_token_2022::{extension::StateWithExtensionsOwned, state::Mint};
use std::sync::Arc;
use tokio::task;

const TRANSFER_TRANSACTION_LABELS: [&str; 5] = [
    // create proof accounts: range, equality, ciphertext validity
    "Create Proof Accounts",
    // verify proof accounts: range
    "Verify Proof Accounts: Range",
    // verify proof accounts: equality, ciphertext validity
    "Verify Proof Accounts: Equality, Ciphertext",
    // transfer
    "Transfer",
    // close proof accounts: equality, ciphertext, range validity
    "Close Proof Accounts",
];

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
    let Some(wallet) =
        db::get_user_wallet_by_pubkey(&state.db, &payload.source, auth_user.telegram_user_id)
            .await?
    else {
        return Err(AppError::not_found(anyhow::anyhow!(
            "Wallet not found or not authorized"
        )));
    };

    if wallet.pubkey != payload.source {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Wallet address does not match provided address"
        )));
    }

    let enabled_confidential_features =
        solana::tokens::get_enabled_confidential_features(state.rpc_client.clone(), &payload.mint)
            .await?;
    if !enabled_confidential_features.contains(&ExtensionType::ConfidentialTransferMint) {
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
                "Recipient confidential token account not found",
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
            "Recipient confidential token account is not configured"
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
    let sender_kp: Arc<dyn solana_signer::Signer + Send + Sync> =
        Arc::new(wallet.signer(&state.kms_client));
    let recipient = payload.recipient;
    let mint = payload.mint;
    let amount = payload.amount;

    // TODO: we can remove this `spawn_blocking` if we remove `ProgramRpcClientSendTransaction`
    let transfer_signatures = task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(with_split_proofs(
            state.rpc_client.clone(),
            sender_kp.clone(),
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

    let transactions = TRANSFER_TRANSACTION_LABELS
        .iter()
        .zip(transfer_signatures.iter())
        .map(|(label, signature)| TransactionResult {
            label: label.to_string(),
            signature: signature.clone(),
        })
        .collect();

    Ok(ApiResponse::new(TransferResponse { transactions }))
}
