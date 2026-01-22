use crate::AppState;
use crate::db;
use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use crate::solana::balance::get_confidential_balances_with_keys;
use crate::solana::tokens::get_maybe_ata;
use crate::solana::utils::confidential_keys_for_mint;
use anyhow::Result;
use axum::extract::Path;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_pubkey::Pubkey;
use std::sync::Arc;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceQuery {
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedBalance {
    #[serde_as(as = "DisplayFromStr")]
    pub pending: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub available: u64,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceResponse {
    /// The wallet address.
    #[serde_as(as = "DisplayFromStr")]
    pub owner: Pubkey,
    /// The mint address.
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
    /// The token account address.
    #[serde_as(as = "DisplayFromStr")]
    pub token_account: Pubkey,
    /// The public balance of the token account.
    #[serde_as(as = "DisplayFromStr")]
    pub public_balance: u64,
    /// The encrypted balance of the token account.
    pub encrypted_balance: EncryptedBalance,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalancePath {
    #[serde_as(as = "DisplayFromStr")]
    pub address: Pubkey,
}

// WARN: do not ship this in production. Anyone can request the balance of any other token account.
pub async fn handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<BalancePath>,
    Query(params): Query<BalanceQuery>,
) -> Result<ApiResponse<BalanceResponse>, AppError> {
    if !db::wallet_exists(&state.db, &path.address).await? {
        return Err(AppError::not_found(anyhow::anyhow!("Wallet not found")));
    }

    let (ata, maybe_ata_account) =
        get_maybe_ata(state.rpc_client.clone(), &path.address, &params.mint).await?;
    if maybe_ata_account.is_none() {
        return Err(AppError::not_found(anyhow::anyhow!(
            "Token account not found"
        )));
    }

    let Some(wallet) = db::get_wallet_by_pubkey(&state.db, &path.address).await? else {
        return Err(AppError::not_found(anyhow::anyhow!("Wallet not found")));
    };

    let confidential_keys = confidential_keys_for_mint(Arc::new(wallet.keypair), &params.mint)?;
    let (pending_balance, available_balance) = get_confidential_balances_with_keys(
        state.rpc_client.clone(),
        &path.address,
        &params.mint,
        &confidential_keys,
    )
    .await?;

    let public_balance = state
        .rpc_client
        .get_token_account_balance(&ata)
        .await
        .map_err(|e| {
            AppError::internal_server_error(anyhow::anyhow!(
                "Failed to get token account balance: {}",
                e
            ))
        })?
        .amount
        .parse::<u64>()
        .map_err(|err| anyhow::anyhow!("Failed to parse ATA balance: {}", err))?;

    Ok(ApiResponse::new(BalanceResponse {
        owner: path.address,
        mint: params.mint,
        token_account: ata,
        public_balance,
        encrypted_balance: EncryptedBalance {
            pending: pending_balance,
            available: available_balance,
        },
    }))
}
