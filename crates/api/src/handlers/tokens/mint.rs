use crate::kms::{KmsKeypair, KmsKeypairIdentifier};
use crate::solana;
use crate::solana::tokens::setup_token_account_with_keys;
use crate::solana::utils::confidential_keys_for_mint;
use crate::{
    AppState, db,
    handlers::{ApiResponse, AppError},
    solana::transaction::build_transaction,
};
use anyhow::Result;
use axum::extract::Path;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_keypair::Keypair;
use solana_keypair::Signature;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use std::sync::Arc;
use tracing::info;

// TODO: make dynamic?
const TOKEN_DECIMALS: u32 = 9;
const DECIMAL_MULTIPLIER: f64 = 1_000_000_000.0; // 10^9

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintTokenRequest {
    /// Mint address
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
    /// Amount to mint
    #[serde_as(as = "DisplayFromStr")]
    pub amount: f64,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintTokenPath {
    #[serde_as(as = "DisplayFromStr")]
    pub address: Pubkey,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintTokenResponse {
    /// The mint address.
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
    /// The signature of the mint transaction.
    #[serde_as(as = "DisplayFromStr")]
    pub signature: Signature,
}

/// Handler for POST /tokens/:address/mint
pub async fn handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<MintTokenPath>,
    Json(payload): Json<MintTokenRequest>,
) -> Result<ApiResponse<MintTokenResponse>, AppError> {
    let adjusted_amount = scale_decimal_amount(payload.amount);
    info!(
        "Minting {:?} of mint={:?} to recipient={:?}",
        adjusted_amount, payload.mint, path.address
    );

    // Validate recipient wallet
    let ata_authority: KmsKeypairIdentifier =
        validate_recipient_wallet(&state.db, &path.address).await?;
    let ata_authority = Arc::new(KmsKeypair::new(
        &state.kms_client,
        ata_authority.key_id,
        ata_authority.pubkey,
    ));

    // Prepare confidential keys and parameters
    let confidential_keys = confidential_keys_for_mint(ata_authority.clone(), &payload.mint)?;
    let confidential_features =
        solana::tokens::get_enabled_confidential_features(state.rpc_client.clone(), &payload.mint)
            .await?;

    let has_confidential_features = !confidential_features.is_empty();
    let confidential_mint_params = if has_confidential_features {
        Some(solana::mint::ConfidentialMintParams {
            destination_keys: &confidential_keys,
            supply_elgamal_keypair: state.elgamal_keypair.clone(),
            supply_aes_key: state.supply_aes_key.clone(),
            auditor_elgamal_pubkey: Some(*state.elgamal_keypair.pubkey()),
        })
    } else {
        None
    };

    // Setup token account if needed
    execute_token_account_setup(
        &state,
        state.global_authority.clone(),
        ata_authority.clone(),
        &payload.mint,
        &confidential_keys,
        has_confidential_features,
    )
    .await?;

    // Execute mint based on confidential status
    let signature = if let Some(params) = confidential_mint_params {
        let receiving_token_account = get_associated_token_address_with_program_id(
            &path.address,
            &payload.mint,
            &spl_token_2022::id(),
        );

        execute_confidential_mint(
            &state,
            state.global_authority.clone(),
            &receiving_token_account,
            &payload.mint,
            adjusted_amount,
            params,
        )
        .await?
    } else {
        execute_standard_mint(
            &state,
            state.global_authority.clone(),
            &path.address,
            &payload.mint,
            adjusted_amount,
        )
        .await?
    };

    Ok(ApiResponse::new(MintTokenResponse {
        mint: payload.mint,
        signature,
    }))
}

fn scale_decimal_amount(amount: f64) -> u64 {
    (amount * DECIMAL_MULTIPLIER) as u64
}

async fn validate_recipient_wallet(
    db: &sqlx::PgPool,
    address: &Pubkey,
) -> Result<KmsKeypairIdentifier> {
    let wallet = db::get_wallet_by_pubkey(db, address)
        .await?
        .ok_or_else(|| AppError::not_found(anyhow::anyhow!("Recipient wallet not found")))?;

    if wallet.pubkey != *address {
        return Err(anyhow::anyhow!(
            "Recipient wallet keypair does not match provided pubkey"
        ));
    }

    Ok(KmsKeypairIdentifier {
        key_id: wallet.kms_key_id,
        pubkey: wallet.pubkey,
    })
}

/// Sends a transaction and confirms it, with consistent error handling.
async fn send_transaction(
    rpc_client: Arc<RpcClient>,
    transaction: &VersionedTransaction,
    context: &str,
) -> Result<Signature, AppError> {
    rpc_client
        .clone()
        .send_and_confirm_transaction(transaction)
        .await
        .map_err(|e| {
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to send and confirm {} transaction: {}",
                context,
                e
            ))
        })
}

/// Executes token account setup if needed, returning whether setup was performed.
async fn execute_token_account_setup(
    state: &AppState,
    global_authority: Arc<dyn Signer + Send + Sync>,
    ata_authority: Arc<dyn Signer + Send + Sync>,
    mint: &Pubkey,
    confidential_keys: &solana::signature_signer::ConfidentialKeys,
    is_confidential: bool,
) -> Result<bool, AppError> {
    let setup_instructions = setup_token_account_with_keys(
        state.rpc_client.clone(),
        &global_authority.pubkey(),
        &ata_authority.pubkey(),
        mint,
        confidential_keys,
    )
    .await?;

    if setup_instructions.instructions.is_empty() {
        return Ok(false);
    }

    if is_confidential {
        info!("Setting up confidential token account for mint={:?}", mint);

        let transaction = build_transaction(
            state.rpc_client.clone(),
            None,
            setup_instructions.instructions,
            global_authority.clone() as Arc<dyn Signer + Send + Sync>,
            vec![ata_authority.clone() as Arc<dyn Signer + Send + Sync>],
        )
        .await?;

        let signature = send_transaction(
            state.rpc_client.clone(),
            &transaction,
            "token account setup",
        )
        .await?;
        info!(
            "Token account setup completed with signature={:?}",
            signature
        );
    }

    Ok(true)
}

/// Executes a confidential mint operation.
async fn execute_confidential_mint(
    state: &AppState,
    global_authority: Arc<dyn Signer + Send + Sync>,
    receiving_token_account: &Pubkey,
    mint: &Pubkey,
    amount: u64,
    params: solana::mint::ConfidentialMintParams<'_>,
) -> Result<Signature, AppError> {
    let pending_txs = solana::mint::build_confidential_mint_transactions(
        state.rpc_client.clone(),
        global_authority.clone(),
        receiving_token_account,
        mint,
        amount,
        params,
    )
    .await?;

    let mut last_signature = Signature::default();
    for pending in pending_txs {
        let transaction = build_transaction(
            state.rpc_client.clone(),
            None,
            pending.instructions,
            global_authority.clone(),
            pending.additional_signers,
        )
        .await?;

        last_signature =
            send_transaction(state.rpc_client.clone(), &transaction, "confidential mint").await?;
    }

    info!(
        "Completed confidential mint for mint={:?} with signature={:?}",
        mint, last_signature
    );

    Ok(last_signature)
}

/// Executes a standard (non-confidential) mint operation.
async fn execute_standard_mint(
    state: &AppState,
    global_authority: Arc<dyn Signer + Send + Sync>,
    recipient_address: &Pubkey,
    mint: &Pubkey,
    amount: u64,
) -> Result<Signature, AppError> {
    info!("Creating standard mint instructions for mint={:?}", mint);

    let mint_instructions = solana::mint::go(
        state.rpc_client.clone(),
        &global_authority.pubkey(),
        global_authority.clone(),
        recipient_address,
        mint,
        amount,
        None,
    )
    .await?;

    let transaction = build_transaction(
        state.rpc_client.clone(),
        None,
        mint_instructions.instructions,
        global_authority.clone(),
        mint_instructions.additional_signers,
    )
    .await?;

    let signature =
        send_transaction(state.rpc_client.clone(), &transaction, "standard mint").await?;

    info!(
        "Completed standard mint for mint={:?} with signature={:?}",
        mint, signature
    );

    Ok(signature)
}
