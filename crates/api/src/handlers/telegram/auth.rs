use crate::AppState;
use crate::db::upsert_telegram_user;
use crate::handlers::{ApiResponse, AppError};
use axum::{Json, extract::State};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use jsonwebtoken::{EncodingKey, Header, encode};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct TelegramAuthRequest {
    #[serde(rename = "initData")]
    pub init_data: String,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct TelegramUser {
    #[serde(rename = "telegramUserId")]
    pub telegram_user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(rename = "firstName", skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(rename = "lastName", skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(rename = "languageCode", skip_serializing_if = "Option::is_none")]
    pub language_code: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct TelegramAuthResponse {
    pub token: String,
    pub user: TelegramUser,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
    #[serde(rename = "hasReservedWallet")]
    pub has_reserved_wallet: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    sub: String,
    telegram_user_id: i64,
    username: Option<String>,
    exp: usize,
    iat: usize,
}

#[derive(Debug, Deserialize)]
struct TelegramInitDataUser {
    id: i64,
    first_name: Option<String>,
    last_name: Option<String>,
    username: Option<String>,
    language_code: Option<String>,
}

/// Verify Telegram WebApp initData
/// https://core.telegram.org/bots/webapps#validating-data-received-via-the-mini-app
fn verify_init_data(init_data: &str, bot_token: &str) -> Result<TelegramUser, AppError> {
    let params: HashMap<String, String> = url::form_urlencoded::parse(init_data.as_bytes())
        .into_owned()
        .collect();

    let hash = params.get("hash").ok_or_else(|| {
        error!("missing hash in initData");
        AppError::bad_request(anyhow::anyhow!("Missing hash in initData"))
    })?;

    // Build data-check-string: sort keys alphabetically, exclude hash
    let mut check_params: Vec<(&str, &str)> = params
        .iter()
        .filter(|(k, _)| *k != "hash")
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    check_params.sort_by(|a, b| a.0.cmp(b.0));

    let data_check_string: String = check_params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    // Create secret key: HMAC-SHA256("WebAppData", bot_token)
    let mut secret_hmac =
        Hmac::<Sha256>::new_from_slice(b"WebAppData").expect("HMAC can take key of any size");
    secret_hmac.update(bot_token.as_bytes());
    let secret_key = secret_hmac.finalize().into_bytes();

    // Calculate hash: HMAC-SHA256(secret_key, data_check_string)
    let mut data_hmac =
        Hmac::<Sha256>::new_from_slice(&secret_key).expect("HMAC can take key of any size");
    data_hmac.update(data_check_string.as_bytes());
    let calculated_hash = hex::encode(data_hmac.finalize().into_bytes());

    if calculated_hash != *hash {
        error!("signature verification failed: hash mismatch");
        return Err(AppError::new(
            anyhow::anyhow!("Invalid initData signature"),
            StatusCode::UNAUTHORIZED,
        ));
    }

    // Optionally check auth_date to prevent replay attacks (e.g., within last hour)
    if let Some(auth_date_str) = params.get("auth_date") {
        if let Ok(auth_date) = auth_date_str.parse::<i64>() {
            let now = Utc::now().timestamp();
            let age_seconds = now - auth_date;
            let max_age_seconds = 3600; // 1 hour

            if age_seconds > max_age_seconds {
                return Err(AppError::new(
                    anyhow::anyhow!("initData expired"),
                    StatusCode::UNAUTHORIZED,
                ));
            }
        }
    }

    // Extract user data
    let user_json = params.get("user").ok_or_else(|| {
        error!("missing user in initData");
        AppError::bad_request(anyhow::anyhow!("Missing user in initData"))
    })?;

    let tg_user: TelegramInitDataUser = serde_json::from_str(user_json).map_err(|e| {
        error!("failed to parse user JSON: {}", e);
        AppError::bad_request(anyhow::anyhow!("Invalid user JSON: {}", e))
    })?;

    Ok(TelegramUser {
        telegram_user_id: tg_user.id,
        username: tg_user.username,
        first_name: tg_user.first_name,
        last_name: tg_user.last_name,
        language_code: tg_user.language_code,
    })
}

fn generate_jwt(user: &TelegramUser, jwt_secret: &str) -> Result<(String, String), AppError> {
    let now = Utc::now();
    let expires_at = now + Duration::hours(24);

    let claims = JwtClaims {
        sub: user.telegram_user_id.to_string(),
        telegram_user_id: user.telegram_user_id,
        username: user.username.clone(),
        exp: expires_at.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::internal_server_error(anyhow::anyhow!("JWT encoding failed: {}", e)))?;

    Ok((token, expires_at.to_rfc3339()))
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TelegramAuthRequest>,
) -> Result<ApiResponse<TelegramAuthResponse>, AppError> {
    let user = if state.dev_mode {
        info!("dev mode enabled, using mock user");
        TelegramUser {
            telegram_user_id: 123456789,
            username: Some("dev_user".to_string()),
            first_name: Some("Dev".to_string()),
            last_name: Some("User".to_string()),
            language_code: Some("en".to_string()),
        }
    } else {
        verify_init_data(&payload.init_data, &state.telegram_bot_token)?
    };

    let upsert_result = upsert_telegram_user(
        &state.db,
        user.telegram_user_id,
        user.username.as_deref(),
        user.first_name.as_deref(),
        user.last_name.as_deref(),
        user.language_code.as_deref(),
    )
    .await
    .map_err(|e| {
        error!("failed to save user to DB: {}", e);
        AppError::internal_server_error(anyhow::anyhow!("Failed to save user: {}", e))
    })?;

    let (token, expires_at) = generate_jwt(&user, &state.jwt_secret)?;

    Ok(ApiResponse::new(TelegramAuthResponse {
        token,
        user,
        expires_at,
        has_reserved_wallet: upsert_result.claimed_reserved_wallet,
    }))
}
