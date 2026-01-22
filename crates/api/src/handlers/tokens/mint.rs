use crate::solana;
use crate::solana::tokens::setup_token_account_with_keys;
use crate::solana::utils::confidential_keys_for_mint;
use crate::{
    AppState, db,
    handlers::{ApiResponse, AppError},
    solana::transaction::build_transaction,
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::Signature;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::Arc;
use tracing::info;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintTokenRequest {
    /// Mint address
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
    /// Recipient address
    #[serde_as(as = "DisplayFromStr")]
    pub recipient: Pubkey,
    /// Amount to mint
    #[serde_as(as = "DisplayFromStr")]
    pub amount: u64,
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

// TODO: not condiential mint atm
// handler is at POST /tokens/:address/mint
pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MintTokenRequest>,
) -> Result<ApiResponse<MintTokenResponse>, AppError> {
    info!(
        "Minting {:?} of mint={:?} to recipient={:?}",
        payload.amount, payload.mint, payload.recipient
    );

    let global_authority = state.global_authority.clone();
    let global_authority_pubkey = global_authority.pubkey();

    let Some(recipient_wallet) = db::get_wallet_by_pubkey(&state.db, &payload.recipient).await?
    else {
        return Err(AppError::not_found(anyhow::anyhow!(
            "Recipient wallet not found"
        )));
    };
    if recipient_wallet.pubkey != payload.recipient {
        return Err(AppError::internal_server_error(anyhow::anyhow!(
            "Recipient wallet keypair does not match provided pubkey"
        )));
    }

    let mut instructions = Vec::new();
    let mut additional_signers = Vec::new();

    let ata_authority = Arc::new(recipient_wallet.keypair);
    let ata_authority_pubkey = ata_authority.pubkey();

    let confidential_keys = confidential_keys_for_mint(ata_authority.clone(), &payload.mint)?;

    // Setup the token account with the derived keys
    let setup_token_account_instructions = setup_token_account_with_keys(
        state.rpc_client.clone(),
        &global_authority_pubkey,
        &ata_authority_pubkey,
        &payload.mint,
        &confidential_keys,
    )
    .await?;

    println!(
        "setup_token_account_instructions ix count: {:?}",
        setup_token_account_instructions.instructions.len()
    );

    println!(
        "setup_token_account_instructions signer count: {:?}",
        setup_token_account_instructions.additional_signers.len()
    );

    if !setup_token_account_instructions.instructions.is_empty() {
        instructions.extend(setup_token_account_instructions.instructions);

        // if !setup_token_account_instructions
        //     .additional_signers
        //     .is_empty()
        // {
        //     additional_signers.extend(setup_token_account_instructions.additional_signers);
        // }

        additional_signers.push(ata_authority.clone() as Arc<dyn Signer + Send + Sync>);
    }

    info!("Create mint instructions for mint={:?}", payload.mint);

    let mint_instructions = solana::mint::go(
        state.rpc_client.clone(),
        &global_authority.pubkey(),
        global_authority.clone(),
        &payload.recipient,
        &payload.mint,
        payload.amount,
    )
    .await?;

    println!(
        "mint_instructions ix count: {:?}",
        mint_instructions.instructions.len()
    );

    println!(
        "mint_instructions signer count: {:?}",
        mint_instructions.additional_signers.len()
    );

    if !mint_instructions.instructions.is_empty() {
        instructions.extend(mint_instructions.instructions);

        if !mint_instructions.additional_signers.is_empty() {
            additional_signers.extend(mint_instructions.additional_signers);
        }
    }

    info!("Build mint transaction for mint={:?}", payload.mint);

    let transaction = build_transaction(
        state.rpc_client.clone(),
        None,
        instructions,
        global_authority.clone(),
        additional_signers,
    )
    .await?;

    info!("Sending mint transaction for mint={:?}", payload.mint);

    let transaction_signature = state
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .map_err(|e| {
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to send and confirm transaction: {}",
                e
            ))
        })?;

    info!(
        "Created mint transaction for mint={:?} with signature={:?}",
        payload.mint, transaction_signature
    );

    Ok(ApiResponse::new(MintTokenResponse {
        mint: payload.mint,
        signature: transaction_signature,
    }))
}
