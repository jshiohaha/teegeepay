use crate::AppState;
use crate::auth::AuthUser;
use crate::handlers::wallets::deposit::TransactionResult;
use crate::handlers::{ApiResponse, AppError};
use crate::solana;
use crate::solana::airdrop::request_airdrop_and_confirm;
use crate::solana::tokens::setup_token_account_with_keys;
use crate::solana::transaction::build_transaction;
use crate::solana::utils::confidential_keys_for_mint;
use anyhow::Result;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::Arc;
use tracing::{error, info};

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelegramTransferRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub source: Pubkey,
    pub telegram_username: String,
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: u64,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipient {
    #[serde_as(as = "DisplayFromStr")]
    pub pubkey: Pubkey,
    pub username: String,
    pub new_wallet: bool,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelegramTransferResponse {
    pub transactions: Vec<TransactionResult>,
    pub recipient: Recipient,
}

struct RecipientInfo {
    wallet: crate::models::Wallet,
    was_new_wallet: bool,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<TelegramTransferRequest>,
) -> Result<ApiResponse<TelegramTransferResponse>, AppError> {
    let sender_wallet =
        super::validate_sender_wallet(&state, &payload.source, auth_user.telegram_user_id).await?;
    super::validate_confidential_mint(&state, &payload.mint).await?;

    let recipient_info = get_or_create_recipient_wallet(&state, &payload.telegram_username)
        .await
        .map_err(AppError::from)?;

    let recipient_pubkey = recipient_info.wallet.pubkey;
    let recipient_keypair = recipient_info.wallet.keypair.clone();
    info!(
        "transfer: telegram username: {}, recipient pubkey: {}",
        payload.telegram_username, recipient_pubkey
    );

    ensure_recipient_confidential_account(
        &state,
        &recipient_pubkey,
        &payload.mint,
        &recipient_keypair,
    )
    .await?;

    let mint_decimals = super::get_mint_decimals(&state, &payload.mint).await?;

    let transfer_signatures = super::execute_transfer(
        state.rpc_client.clone(),
        sender_wallet.keypair.clone(),
        &recipient_pubkey,
        payload.amount,
        payload.mint,
        mint_decimals,
    )
    .await?;

    let transactions = super::format_transfer_results(&transfer_signatures);

    Ok(ApiResponse::new(TelegramTransferResponse {
        transactions,
        recipient: Recipient {
            pubkey: recipient_pubkey,
            username: payload.telegram_username,
            new_wallet: recipient_info.was_new_wallet,
        },
    }))
}

async fn get_or_create_recipient_wallet(
    state: &AppState,
    telegram_username: &str,
) -> Result<RecipientInfo> {
    let existing_wallet =
        crate::db::get_wallet_by_telegram_username(&state.db, telegram_username).await?;
    if let Some(wallet) = existing_wallet {
        info!("found existing wallet for username: {}", telegram_username);
        return Ok(RecipientInfo {
            wallet,
            was_new_wallet: false,
        });
    }

    info!(
        "creating new reserved wallet for username: {}",
        telegram_username
    );
    let keypair = Keypair::new();
    let wallet =
        crate::db::get_or_create_wallet_for_username(&state.db, telegram_username, &keypair)
            .await
            .map_err(|e| anyhow::anyhow!("failed to create wallet for recipient: {}", e))?;

    request_airdrop_and_confirm(state.rpc_client.clone(), &wallet.pubkey, 1 * 10_u64.pow(9))
        .await
        .map_err(|e| anyhow::anyhow!("failed to fund recipient wallet: {}", e))?;

    Ok(RecipientInfo {
        wallet,
        was_new_wallet: true,
    })
}

async fn ensure_recipient_confidential_account(
    state: &AppState,
    recipient_pubkey: &Pubkey,
    mint: &Pubkey,
    recipient_keypair: &Arc<Keypair>,
) -> Result<(), AppError> {
    let requires_recipient_setup = {
        let (_, maybe_recipient_ata_account) =
            solana::tokens::get_maybe_ata(state.rpc_client.clone(), recipient_pubkey, mint).await?;

        if maybe_recipient_ata_account.is_none() {
            true
        } else {
            solana::tokens::ata_has_confidential_transfer_extension(
                maybe_recipient_ata_account,
                recipient_pubkey,
                mint,
            )?
        }
    };

    if !requires_recipient_setup {
        return Ok(());
    }

    let confidential_keys =
        confidential_keys_for_mint(recipient_keypair.clone(), mint).map_err(|e| {
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to derive confidential keys: {}",
                e
            ))
        })?;

    let setup_instructions = setup_token_account_with_keys(
        state.rpc_client.clone(),
        &state.global_authority.pubkey(),
        recipient_pubkey,
        mint,
        &confidential_keys,
    )
    .await
    .map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!(
            "Failed to setup recipient token account: {}",
            e
        ))
    })?;

    if setup_instructions.instructions.is_empty() {
        return Ok(());
    }

    let mut additional_signers: Vec<Arc<dyn Signer + Send + Sync>> =
        vec![recipient_keypair.clone()];
    additional_signers.extend(setup_instructions.additional_signers);

    let transaction = build_transaction(
        state.rpc_client.clone(),
        None,
        setup_instructions.instructions,
        state.global_authority.clone(),
        additional_signers,
    )
    .await
    .map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to build setup transaction: {}", e))
    })?;

    state
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .map_err(|e| {
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to setup recipient token account: {}",
                e
            ))
        })?;

    Ok(())
}
