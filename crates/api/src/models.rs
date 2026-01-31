use crate::kms::KmsKeypair;
use aws_sdk_kms::Client as KmsClient;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub external_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Wallet {
    pub id: i64,
    pub user_id: i64,
    pub pubkey: Pubkey,
    pub kms_key_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Wallet {
    pub fn signer(&self, kms_client: &KmsClient) -> KmsKeypair {
        KmsKeypair::new(kms_client, self.kms_key_id.clone(), self.pubkey)
    }
}
