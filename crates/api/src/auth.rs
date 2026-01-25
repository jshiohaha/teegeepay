use crate::AppState;
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
};
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn, error, debug};

const DEV_MOCK_TOKEN: &str = "dev_mock_token_for_local_testing";
const DEV_MOCK_USER_ID: i64 = 123456789;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AuthClaims {
    pub sub: String,
    pub telegram_user_id: i64,
    pub username: Option<String>,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub telegram_user_id: i64,
    pub username: Option<String>,
}

impl From<AuthClaims> for AuthUser {
    fn from(claims: AuthClaims) -> Self {
        Self {
            telegram_user_id: claims.telegram_user_id,
            username: claims.username,
        }
    }
}

pub struct AuthError {
    pub message: String,
    pub status: StatusCode,
}

impl AuthError {
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::UNAUTHORIZED,
        }
    }
}

impl axum::response::IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
    }
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let path = parts.uri.path();
        info!("[AUTH_MW] ========== AUTH MIDDLEWARE ==========");
        info!("[AUTH_MW] Request path: {}, dev_mode: {}", path, state.dev_mode);
        
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        
        debug!("[AUTH_MW] Authorization header present: {}", auth_header.is_some());
        
        let auth_header = auth_header
            .ok_or_else(|| {
                warn!("[AUTH_MW] Missing Authorization header for path: {}", path);
                AuthError::unauthorized("Missing Authorization header")
            })?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| {
                warn!("[AUTH_MW] Invalid Authorization header format for path: {}", path);
                AuthError::unauthorized("Invalid Authorization header format")
            })?;

        debug!("[AUTH_MW] Token length: {}, first 20 chars: {}...", 
            token.len(), 
            &token[..token.len().min(20)]);

        // Dev mode bypass: accept mock token for local development
        if state.dev_mode && token == DEV_MOCK_TOKEN {
            info!("[AUTH_MW] Dev mode: accepting mock token");
            return Ok(AuthUser {
                telegram_user_id: DEV_MOCK_USER_ID,
                username: Some("dev_user".to_string()),
            });
        }

        let bypass_token = std::env::var("BYPASS_AUTH_TOKEN").unwrap_or_default();
        if !bypass_token.is_empty() && token == bypass_token {
            info!("[AUTH_MW] Bypass token accepted");
            return Ok(AuthUser::from(AuthClaims::default()));
        }

        debug!("[AUTH_MW] Attempting JWT decode");
        let token_data = decode::<AuthClaims>(
            token,
            &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| {
            error!("[AUTH_MW] JWT decode failed for path {}: {}", path, e);
            error!("[AUTH_MW] Token (first 50 chars): {}...", &token[..token.len().min(50)]);
            AuthError::unauthorized(format!("Invalid token: {}", e))
        })?;

        info!("[AUTH_MW] JWT valid - user_id: {}, username: {:?}, exp: {}", 
            token_data.claims.telegram_user_id,
            token_data.claims.username,
            token_data.claims.exp);
        info!("[AUTH_MW] ========== AUTH SUCCESS ==========");

        Ok(AuthUser::from(token_data.claims))
    }
}
