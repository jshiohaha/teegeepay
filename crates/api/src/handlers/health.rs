use crate::handlers::{ApiResponse, AppError};

pub async fn handler() -> Result<ApiResponse<&'static str>, AppError> {
    Ok(ApiResponse::new("OK"))
}
