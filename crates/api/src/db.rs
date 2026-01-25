use crate::models::Wallet;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use sqlx::{PgConnection, PgPool, prelude::FromRow};
use std::str::FromStr;
use tracing::{info, error, debug};

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

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct TelegramUserRow {
    pub id: i64,
    pub user_id: String,
    pub telegram_user_id: Option<i64>,
    pub telegram_username: Option<String>,
    pub telegram_first_name: Option<String>,
    pub telegram_last_name: Option<String>,
    pub telegram_language_code: Option<String>,
    pub telegram_auth_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[allow(dead_code)]
pub async fn get_user_by_user_id(pool: &PgPool, user_id: &str) -> Result<Option<TelegramUserRow>> {
    let user = sqlx::query_as::<_, TelegramUserRow>(
        r#"
        SELECT *
        FROM users
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

pub async fn upsert_telegram_user(
    pool: &PgPool,
    telegram_user_id: i64,
    username: Option<&str>,
    first_name: Option<&str>,
    last_name: Option<&str>,
    language_code: Option<&str>,
) -> Result<TelegramUserRow> {
    let user_id_str = format!("tg:{}", telegram_user_id);

    let user = sqlx::query_as::<_, TelegramUserRow>(
        r#"
        INSERT INTO users (
            user_id,
            telegram_user_id,
            telegram_username,
            telegram_first_name,
            telegram_last_name,
            telegram_language_code,
            telegram_auth_date,
            created_at,
            updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW(), NOW())
        ON CONFLICT (telegram_user_id) DO UPDATE SET
            telegram_username = EXCLUDED.telegram_username,
            telegram_first_name = EXCLUDED.telegram_first_name,
            telegram_last_name = EXCLUDED.telegram_last_name,
            telegram_language_code = EXCLUDED.telegram_language_code,
            telegram_auth_date = NOW(),
            updated_at = NOW()
        RETURNING *
        "#,
    )
    .bind(&user_id_str)
    .bind(telegram_user_id)
    .bind(username)
    .bind(first_name)
    .bind(last_name)
    .bind(language_code)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

/// Create a wallet for an existing Telegram user.
/// Returns the wallet id, or an error if the user doesn't exist.
pub async fn create_wallet_for_telegram_user(
    pool: &PgPool,
    telegram_user_id: i64,
    pubkey: &Pubkey,
    keypair: &Keypair,
) -> Result<i64> {
    let user_id_str = format!("tg:{}", telegram_user_id);
    info!("[DB] create_wallet_for_telegram_user - telegram_user_id: {}, user_id_str: {}, pubkey: {}", 
        telegram_user_id, user_id_str, pubkey);

    let wallet_id = sqlx::query_scalar::<_, i64>(
        r#"
        WITH upsert_user AS (
            INSERT INTO users (user_id, telegram_user_id, created_at, updated_at)
            VALUES ($1, $2, NOW(), NOW())
            ON CONFLICT (telegram_user_id) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                updated_at = NOW()
            RETURNING id
        )
        INSERT INTO wallets (user_id, pubkey, keypair, created_at, updated_at)
        SELECT upsert_user.id, $3, $4, NOW(), NOW()
        FROM upsert_user
        RETURNING id
        "#,
    )
    .bind(&user_id_str)
    .bind(telegram_user_id)
    .bind(pubkey.to_string())
    .bind(keypair.to_base58_string())
    .fetch_one(pool)
    .await
    .map_err(|e| {
        error!("[DB] create_wallet_for_telegram_user failed: {}", e);
        e
    })?;

    info!("[DB] create_wallet_for_telegram_user success - wallet_id: {}", wallet_id);
    Ok(wallet_id)
}

#[allow(dead_code)]
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

/// Get wallet by pubkey, but only if it belongs to the given user.
/// Returns None if wallet doesn't exist OR if it belongs to a different user.
pub async fn get_user_wallet_by_pubkey(
    pool: &PgPool,
    pubkey: &Pubkey,
    telegram_user_id: i64,
) -> Result<Option<Wallet>> {
    let user_id_str = format!("tg:{}", telegram_user_id);

    let wallet = sqlx::query_as::<_, WalletRow>(
        r#"
        SELECT w.*
        FROM wallets w
        JOIN users u ON w.user_id = u.id
        WHERE w.pubkey = $1 AND u.user_id = $2
        "#,
    )
    .bind(pubkey.to_string())
    .bind(&user_id_str)
    .fetch_optional(pool)
    .await?;

    if wallet.is_none() {
        return Err(anyhow::anyhow!(
            "User does not have a wallet the provided pubkey"
        ));
    }

    wallet
        .map(|w| Wallet::try_from(w).map_err(|e| anyhow::anyhow!("Failed to parse wallet: {}", e)))
        .transpose()
}

#[allow(dead_code)]
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

pub async fn get_wallets_for_telegram_user(
    pool: &PgPool,
    telegram_user_id: i64,
) -> Result<Vec<Pubkey>> {
    let user_id_str = format!("tg:{}", telegram_user_id);
    info!("[DB] get_wallets_for_telegram_user - telegram_user_id: {}, user_id_str: {}", 
        telegram_user_id, user_id_str);

    let wallets = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT w.pubkey
        FROM wallets w
        JOIN users u ON w.user_id = u.id
        WHERE u.user_id = $1
        "#,
    )
    .bind(&user_id_str)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("[DB] get_wallets_for_telegram_user query failed: {}", e);
        e
    })?;

    info!("[DB] get_wallets_for_telegram_user - found {} wallets", wallets.len());
    for (i, w) in wallets.iter().enumerate() {
        debug!("[DB] wallet[{}]: {}", i, w.0);
    }

    Ok(wallets
        .into_iter()
        .map(|w| {
            Pubkey::from_str(&w.0).map_err(|e| anyhow::anyhow!("Failed to parse pubkey: {}", e))
        })
        .collect::<Result<Vec<_>>>()?)
}
