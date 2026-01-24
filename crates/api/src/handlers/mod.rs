pub mod convert;
pub mod health;
pub mod telegram;
pub mod tokens;
pub mod transfers;
pub mod wallets;

use axum::{
    Json,
    response::{IntoResponse, Response},
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BaseApiResponse<T> {
    pub data: T,
}

impl<T: Default> BaseApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T> IntoResponse for BaseApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

pub type ApiResponse<T> = BaseApiResponse<T>;

#[derive(Debug, Error)]
#[error("{inner}")]
pub struct AppError {
    inner: anyhow::Error,
    status: StatusCode,
}

impl AppError {
    pub fn new(error: impl Into<anyhow::Error>, status: StatusCode) -> Self {
        Self {
            inner: error.into(),
            status,
        }
    }

    pub fn internal_server_error(error: impl Into<anyhow::Error>) -> Self {
        Self::new(error, StatusCode::INTERNAL_SERVER_ERROR)
    }

    pub fn bad_request(error: impl Into<anyhow::Error>) -> Self {
        Self::new(error, StatusCode::BAD_REQUEST)
    }

    pub fn not_found(error: impl Into<anyhow::Error>) -> Self {
        Self::new(error, StatusCode::NOT_FOUND)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let error_message = self.inner.to_string();

        if error_message.is_empty() {
            (self.status, "server error").into_response()
        } else {
            (self.status, error_message).into_response()
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::internal_server_error(err)
    }
}
