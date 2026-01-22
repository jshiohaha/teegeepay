use crate::models::Wallet;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use sqlx::{PgConnection, PgPool, prelude::FromRow};
use std::str::FromStr;

pub async fn create_wallet(
    tx: &mut PgConnection,
    user_id: i64,
    pubkey: &Pubkey,
    keypair: &Keypair,
) -> Result<i64> {
    let wallet_id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO wallets (
            user_id,
            pubkey,
            keypair,
            created_at,
            updated_at
        )
        VALUES ($1, $2, $3, NOW(), NOW())
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(pubkey.to_string())
    .bind(keypair.to_base58_string())
    .fetch_one(tx.as_mut())
    .await?;

    Ok(wallet_id)
}

pub async fn create_user(tx: &mut PgConnection, user_id: &str) -> Result<i64> {
    let user_id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO users (
            user_id,
            created_at,
            updated_at
        )
        VALUES ($1, NOW(), NOW())
        RETURNING id
        "#,
    )
    .bind(user_id)
    .fetch_one(tx.as_mut())
    .await?;

    Ok(user_id)
}

pub async fn create_user_and_wallet(
    pool: PgPool,
    external_user_id: &str,
    pubkey: &Pubkey,
    keypair: &Keypair,
) -> Result<(i64, i64)> {
    let mut tx = pool.begin().await?;

    let user_id = create_user(&mut *tx, external_user_id).await?;
    let wallet_id = create_wallet(&mut *tx, user_id, pubkey, keypair).await?;

    tx.commit().await?;

    Ok((user_id, wallet_id))
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct WalletRow {
    pub id: i64,
    pub user_id: i64,
    pub pubkey: String,
    pub keypair: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<WalletRow> for Wallet {
    type Error = anyhow::Error;

    fn try_from(wallet: WalletRow) -> Result<Self, Self::Error> {
        Ok(Wallet {
            id: wallet.id,
            user_id: wallet.user_id,
            pubkey: Pubkey::from_str(&wallet.pubkey)
                .map_err(|e| anyhow::anyhow!("Failed to parse pubkey: {}", e))?,
            keypair: Keypair::from_base58_string(&wallet.keypair),
            created_at: wallet.created_at,
            updated_at: wallet.updated_at,
        })
    }
}

#[allow(dead_code)]
pub async fn get_wallet_by_user_id(
    pool: &PgPool,
    external_user_id: &str,
) -> Result<Option<Wallet>> {
    let wallet = sqlx::query_as::<_, WalletRow>(
        r#"
        SELECT w.*
        FROM wallets w
        JOIN users u ON w.user_id = u.id
        WHERE u.user_id = $1
        "#,
    )
    .bind(external_user_id)
    .fetch_optional(pool)
    .await?;

    wallet
        .map(|w| Wallet::try_from(w).map_err(|e| anyhow::anyhow!("Failed to parse wallet: {}", e)))
        .transpose()
}

pub async fn get_wallet_by_pubkey(pool: &PgPool, pubkey: &Pubkey) -> Result<Option<Wallet>> {
    let wallet = sqlx::query_as::<_, WalletRow>(
        r#"
        SELECT *
        FROM wallets w
        WHERE w.pubkey = $1
        "#,
    )
    .bind(pubkey.to_string())
    .fetch_optional(pool)
    .await?;

    wallet
        .map(|w| Wallet::try_from(w).map_err(|e| anyhow::anyhow!("Failed to parse wallet: {}", e)))
        .transpose()
}

pub async fn wallet_exists(pool: &PgPool, pubkey: &Pubkey) -> Result<bool> {
    let result = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(SELECT 1 FROM wallets WHERE pubkey = $1)
        "#,
    )
    .bind(pubkey.to_string())
    .fetch_one(pool)
    .await?;

    Ok(result)
}

pub async fn get_all_wallets(pool: &PgPool) -> Result<Vec<Pubkey>> {
    let wallets = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT pubkey
        FROM wallets
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(wallets
        .into_iter()
        .map(|w| {
            Pubkey::from_str(&w.0).map_err(|e| anyhow::anyhow!("Failed to parse pubkey: {}", e))
        })
        .collect::<Result<Vec<_>>>()?)
}
