use crate::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::handlers::wallets::deposit::TransactionResult;
use crate::handlers::{ApiResponse, AppError};
use crate::solana;
use crate::solana::airdrop::request_airdrop_and_confirm;
use crate::solana::tokens::setup_token_account_with_keys;
use crate::solana::transaction::build_transaction;
use crate::solana::transfer::with_split_proofs;
use crate::solana::utils::confidential_keys_for_mint;
use anyhow::Result;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_keypair::{Keypair, Signature};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use spl_token_2022::extension::ExtensionType;
use spl_token_2022::{extension::StateWithExtensionsOwned, state::Mint};
use std::sync::Arc;
use tokio::task;
use tracing::{error, info};

const TRANSFER_TRANSACTION_LABELS: [&str; 5] = [
    "Create Proof Accounts",
    "Verify Proof Accounts: Range",
    "Verify Proof Accounts: Equality, Ciphertext",
    "Transfer",
    "Close Proof Accounts",
];

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

struct TransferContext {
    recipient_pubkey: Pubkey,
    mint_decimals: u8,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<TelegramTransferRequest>,
) -> Result<ApiResponse<TelegramTransferResponse>, AppError> {
    let sender_wallet =
        validate_sender_wallet(&state, &payload, auth_user.telegram_user_id).await?;
    validate_confidential_mint(&state, payload.mint).await?;

    let recipient_info = get_or_create_recipient_wallet(&state, &payload.telegram_username)
        .await
        .map_err(AppError::from)?;

    let transfer_context =
        prepare_recipient_transfer_context(&state, &payload, &recipient_info.wallet)
            .await
            .map_err(AppError::from)?;

    let sender_kp = sender_wallet.keypair.clone();
    let transfer_signatures = execute_transfer(
        state.rpc_client.clone(),
        sender_kp,
        &transfer_context.recipient_pubkey,
        payload.amount,
        payload.mint,
        transfer_context.mint_decimals,
    )
    .await?;

    let transactions = format_transfer_results(&transfer_signatures);

    Ok(ApiResponse::new(TelegramTransferResponse {
        transactions,
        recipient: Recipient {
            pubkey: transfer_context.recipient_pubkey,
            username: payload.telegram_username,
            new_wallet: recipient_info.was_new_wallet,
        },
    }))
}

async fn validate_sender_wallet(
    state: &AppState,
    payload: &TelegramTransferRequest,
    telegram_user_id: i64,
) -> Result<crate::models::Wallet, AppError> {
    let Some(sender_wallet) =
        db::get_user_wallet_by_pubkey(&state.db, &payload.source, telegram_user_id).await?
    else {
        return Err(AppError::not_found(anyhow::anyhow!(
            "Wallet not found or not authorized"
        )));
    };

    if sender_wallet.pubkey != payload.source {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Wallet address does not match provided address"
        )));
    }

    Ok(sender_wallet)
}

async fn validate_confidential_mint(state: &AppState, mint: Pubkey) -> Result<(), AppError> {
    let enabled_confidential_features =
        solana::tokens::get_enabled_confidential_features(state.rpc_client.clone(), &mint).await?;
    if !enabled_confidential_features.contains(&ExtensionType::ConfidentialTransferMint) {
        return Err(AppError::bad_request(anyhow::anyhow!(
            "Mint is not confidential"
        )));
    }

    Ok(())
}

async fn get_or_create_recipient_wallet(
    state: &AppState,
    telegram_username: &str,
) -> Result<RecipientInfo> {
    let existing_wallet = db::get_wallet_by_telegram_username(&state.db, telegram_username).await?;
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
    let wallet = db::get_or_create_wallet_for_username(&state.db, telegram_username, &keypair)
        .await
        .map_err(|e| {
            error!("failed to create reserved wallet: {}", e);
            anyhow::anyhow!("failed to create wallet for recipient: {}", e)
        })?;

    request_airdrop_and_confirm(state.rpc_client.clone(), &wallet.pubkey, 1 * 10_u64.pow(9))
        .await
        .map_err(|e| anyhow::anyhow!("failed to fund recipient wallet: {}", e))?;

    Ok(RecipientInfo {
        wallet,
        was_new_wallet: true,
    })
}

async fn prepare_recipient_transfer_context(
    state: &AppState,
    payload: &TelegramTransferRequest,
    recipient_wallet: &crate::models::Wallet,
) -> Result<TransferContext, AppError> {
    let recipient_pubkey = recipient_wallet.pubkey;
    let recipient_keypair = recipient_wallet.keypair.clone();
    info!(
        "transfer: telegram username: {}, recipient pubkey: {}",
        payload.telegram_username, recipient_pubkey
    );

    ensure_recipient_confidential_account(
        state,
        &recipient_pubkey,
        payload.mint,
        &recipient_keypair,
    )
    .await?;
    let mint_decimals = get_mint_decimals(state, payload.mint).await?;

    Ok(TransferContext {
        recipient_pubkey,
        mint_decimals,
    })
}

async fn ensure_recipient_confidential_account(
    state: &AppState,
    recipient_pubkey: &Pubkey,
    mint: Pubkey,
    recipient_keypair: &Arc<Keypair>,
) -> Result<(), AppError> {
    let requires_recipient_setup = {
        let (_, maybe_recipient_ata_account) =
            solana::tokens::get_maybe_ata(state.rpc_client.clone(), recipient_pubkey, &mint)
                .await?;

        if maybe_recipient_ata_account.is_none() {
            true
        } else {
            solana::tokens::ata_has_confidential_transfer_extension(
                maybe_recipient_ata_account,
                recipient_pubkey,
                &mint,
            )?
        }
    };

    if !requires_recipient_setup {
        return Ok(());
    }

    let confidential_keys =
        confidential_keys_for_mint(recipient_keypair.clone(), &mint).map_err(|e| {
            error!("fFailed to derive confidential keys: {}", e);
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to derive confidential keys: {}",
                e
            ))
        })?;

    let setup_instructions = setup_token_account_with_keys(
        state.rpc_client.clone(),
        &state.global_authority.pubkey(),
        recipient_pubkey,
        &mint,
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
        error!("[TG_TRANSFER] Failed to build setup transaction: {}", e);
        AppError::internal_server_error(anyhow::anyhow!("Failed to build setup transaction: {}", e))
    })?;

    state
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .map_err(|e| {
            error!("[TG_TRANSFER] Failed to send setup transaction: {}", e);
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to setup recipient token account: {}",
                e
            ))
        })?;

    info!("[TG_TRANSFER] Recipient token account setup complete");
    Ok(())
}

async fn get_mint_decimals(state: &AppState, mint: Pubkey) -> Result<u8, AppError> {
    let mint_account = state.rpc_client.get_account(&mint).await.map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Mint account not found: {:?}", e))
    })?;
    let mint_state = StateWithExtensionsOwned::<Mint>::unpack(mint_account.data).map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to unpack mint account: {:?}", e))
    })?;
    Ok(mint_state.base.decimals)
}

async fn execute_transfer(
    rpc_client: Arc<RpcClient>,
    sender_kp: Arc<Keypair>,
    recipient_pubkey: &Pubkey,
    amount: u64,
    mint: Pubkey,
    mint_decimals: u8,
) -> Result<Vec<Signature>, AppError> {
    let rpc_client = rpc_client.clone();
    let sender_kp = sender_kp.clone();
    let recipient_pubkey = recipient_pubkey.clone();

    task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(with_split_proofs(
            rpc_client.clone(),
            sender_kp,
            &recipient_pubkey,
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
    })
}

fn format_transfer_results(transfer_signatures: &[Signature]) -> Vec<TransactionResult> {
    TRANSFER_TRANSACTION_LABELS
        .iter()
        .zip(transfer_signatures.iter())
        .map(|(label, signature)| TransactionResult {
            label: label.to_string(),
            signature: signature.clone(),
        })
        .collect()
}
