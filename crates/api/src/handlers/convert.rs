use crate::handlers::ApiResponse;
use crate::handlers::AppError;
use axum::Json;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;
use solana_keypair::Keypair;
use solana_signer::Signer;
use tracing::info;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    #[serde_as(as = "serde_with::Bytes")]
    pub bytes: Vec<u8>,
}

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResponse {
    pub keypair: String,
}

// Warn: utility endpoint, no one should use this. It logs the keypair.
pub async fn handler(
    Json(payload): Json<ConvertRequest>,
) -> Result<ApiResponse<ConvertResponse>, AppError> {
    let kp = Keypair::try_from(payload.bytes.as_slice())
        .map_err(|_| AppError::bad_request(anyhow::anyhow!("expected 32 bytes for keypair")))?;

    info!(
        "pubkey: {:?}, keypair: {:?}",
        kp.pubkey(),
        kp.to_base58_string()
    );

    Ok(ApiResponse::new(ConvertResponse {
        keypair: kp.to_base58_string(),
    }))
}
