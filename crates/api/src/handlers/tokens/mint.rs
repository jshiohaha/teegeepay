use crate::solana;
use crate::solana::tokens::setup_token_account_with_keys;
use crate::solana::utils::confidential_keys_for_mint;
use crate::{
    AppState, db,
    handlers::{ApiResponse, AppError},
    solana::transaction::build_transaction,
};
use axum::extract::Path;
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::Signature;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use std::sync::Arc;
use tracing::info;

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

// TODO: not condiential mint atm
// handler is at POST /tokens/:address/mint
pub async fn handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<MintTokenPath>,
    Json(payload): Json<MintTokenRequest>,
) -> Result<ApiResponse<MintTokenResponse>, AppError> {
    let adjusted_amount = (payload.amount * 10_f64.powf(9.0)) as u64;
    info!(
        "Minting {:?} of mint={:?} to recipient={:?}",
        adjusted_amount, payload.mint, path.address
    );

    let Some(recipient_wallet) = db::get_wallet_by_pubkey(&state.db, &path.address).await? else {
        return Err(AppError::not_found(anyhow::anyhow!(
            "Recipient wallet not found"
        )));
    };

    if recipient_wallet.pubkey != path.address {
        return Err(AppError::internal_server_error(anyhow::anyhow!(
            "Recipient wallet keypair does not match provided pubkey"
        )));
    }

    let global_authority = state.global_authority.clone();
    let global_authority_pubkey = global_authority.pubkey();

    let ata_authority = Arc::new(recipient_wallet.keypair);
    let ata_authority_pubkey = ata_authority.pubkey();

    let confidential_keys = confidential_keys_for_mint(ata_authority.clone(), &payload.mint)?;
    let is_confidential_mint =
        solana::tokens::is_confidential_mint_enabled(state.rpc_client.clone(), &payload.mint)
            .await?;
    // let is_confidential_mintburn =
    //     solana::tokens::is_confidential_mintburn_enabled(state.rpc_client.clone(), &payload.mint)
    //         .await?;
    let confidential_mint_params = if is_confidential_mint {
        Some(solana::mint::ConfidentialMintParams {
            destination_keys: &confidential_keys,
            supply_elgamal_keypair: state.elgamal_keypair.clone(),
            supply_aes_key: state.supply_aes_key.clone(),
            auditor_elgamal_pubkey: Some(*state.elgamal_keypair.pubkey()),
        })
    } else {
        None
    };
    let receiving_token_account = get_associated_token_address_with_program_id(
        &path.address,
        &payload.mint,
        &spl_token_2022::id(),
    );

    let mut instructions = Vec::new();
    let mut additional_signers = Vec::new();

    // Setup the token account with the derived keys
    let setup_token_account_instructions = setup_token_account_with_keys(
        state.rpc_client.clone(),
        &global_authority_pubkey,
        &ata_authority_pubkey,
        &payload.mint,
        &confidential_keys,
    )
    .await?;

    if !setup_token_account_instructions.instructions.is_empty() {
        instructions.extend(setup_token_account_instructions.instructions);

        // Note: additional_signers removed in prepration for returning ix's to the user
        additional_signers.push(ata_authority.clone() as Arc<dyn Signer + Send + Sync>);
    }

    if confidential_mint_params.is_some() {
        let tx = build_transaction(
            state.rpc_client.clone(),
            None,
            instructions,
            global_authority.clone(),
            additional_signers,
        )
        .await?;

        info!("Sending mint transaction for mint={:?}", payload.mint);

        let signature = state
            .rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .map_err(|e| {
                AppError::internal_server_error(anyhow::anyhow!(
                    "Failed to send and confirm transaction: {}",
                    e
                ))
            })?;

        println!("setup token account transaction signature: {:?}", signature);

        instructions = vec![];
        additional_signers = vec![];
    }

    if let Some(conf_params) = confidential_mint_params {
        let pending_txs = solana::mint::build_confidential_mint_transactions(
            state.rpc_client.clone(),
            global_authority.clone(),
            &receiving_token_account,
            &payload.mint,
            adjusted_amount,
            conf_params,
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

            last_signature = state
                .rpc_client
                .send_and_confirm_transaction(&transaction)
                .await
                .map_err(|e| {
                    AppError::internal_server_error(anyhow::anyhow!(
                        "Failed to send and confirm transaction: {}",
                        e
                    ))
                })?;
        }

        return Ok(ApiResponse::new(MintTokenResponse {
            mint: payload.mint,
            signature: last_signature,
        }));
    }

    info!("Create mint instructions for mint={:?}", payload.mint);

    let mint_instructions = solana::mint::go(
        state.rpc_client.clone(),
        &global_authority.pubkey(),
        global_authority.clone(),
        &path.address,
        &payload.mint,
        adjusted_amount,
        None,
    )
    .await?;

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
