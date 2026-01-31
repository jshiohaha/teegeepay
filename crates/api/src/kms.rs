use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_kms::{
    Client,
    error::SdkError,
    operation::{create_alias::CreateAliasError, sign::SignError},
    types::{KeySpec, KeyUsageType, MessageType, SigningAlgorithmSpec},
};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_signer::{Signer, SignerError};
use std::convert::TryInto;
use tokio::{runtime::Handle, task};

/// Create a shared AWS KMS client using environment configuration.
pub async fn create_kms_client() -> Client {
    let config = aws_config::defaults(BehaviorVersion::latest()).load().await;
    Client::new(&config)
}

/// Create a new Ed25519 key in AWS KMS and associate it with the provided alias.
pub async fn create_kms_ed25519_key(kms: &Client, alias: &str) -> Result<String> {
    let key = kms
        .create_key()
        .key_usage(KeyUsageType::SignVerify)
        .key_spec(KeySpec::EccNistEdwards25519)
        .description("CypherPay Solana wallet key (Ed25519)")
        .send()
        .await?;

    let key_id = key
        .key_metadata()
        .map(|meta| meta.key_id().to_string())
        .ok_or_else(|| anyhow::anyhow!("KMS response missing key metadata"))?;

    let alias_name = if alias.starts_with("alias/") {
        alias.to_string()
    } else {
        format!("alias/{alias}")
    };

    if let Err(err) = kms
        .create_alias()
        .alias_name(alias_name)
        .target_key_id(&key_id)
        .send()
        .await
    {
        if !alias_already_exists(&err) {
            return Err(err.into());
        }
    }

    Ok(key_id)
}

fn alias_already_exists(err: &SdkError<CreateAliasError>) -> bool {
    if let SdkError::ServiceError(service_err) = err {
        service_err.err().is_already_exists_exception()
    } else {
        false
    }
}

/// Fetch the DER-encoded public key for a KMS key and convert it to a Solana `Pubkey`.
pub async fn solana_pubkey_from_kms(kms: &Client, key_id: &str) -> Result<Pubkey> {
    let resp = kms.get_public_key().key_id(key_id).send().await?;
    let der_bytes = resp
        .public_key()
        .ok_or_else(|| anyhow::anyhow!("KMS response missing public key"))?;

    if der_bytes.as_ref().len() < 32 {
        return Err(anyhow::anyhow!(
            "DER public key shorter than 32 bytes: {}",
            der_bytes.as_ref().len()
        ));
    }
    let raw_pubkey: [u8; 32] = der_bytes.as_ref()[der_bytes.as_ref().len() - 32..]
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid DER payload length for pubkey"))?;
    Ok(Pubkey::from(raw_pubkey))
}

#[derive(Clone)]
pub struct KmsKeypairIdentifier {
    pub key_id: String,
    pub pubkey: Pubkey,
}

/// Signer implementation backed by AWS KMS.
#[derive(Clone)]
pub struct KmsKeypair {
    kms: Client,
    key_id: String,
    pubkey: Pubkey,
}

impl KmsKeypair {
    pub fn new(kms: &Client, key_id: impl Into<String>, pubkey: Pubkey) -> Self {
        Self {
            kms: kms.clone(),
            key_id: key_id.into(),
            pubkey,
        }
    }

    async fn sign_message_async(&self, message: &[u8]) -> Result<Signature, SignerError> {
        let response = self
            .kms
            .sign()
            .key_id(&self.key_id)
            .message(message.to_vec().into())
            .message_type(MessageType::Raw)
            .signing_algorithm(SigningAlgorithmSpec::Ed25519Sha512)
            .send()
            .await
            .map_err(|err| map_sign_error(&err))?;

        let signature_bytes = response
            .signature()
            .ok_or_else(|| SignerError::Custom("KMS response missing signature".into()))?;

        Signature::try_from(signature_bytes.as_ref())
            .map_err(|_| SignerError::Custom("invalid Ed25519 signature returned by KMS".into()))
    }

    fn block_on_sign<F>(&self, future: F) -> Result<Signature, SignerError>
    where
        F: std::future::Future<Output = Result<Signature, SignerError>>,
    {
        if let Ok(handle) = Handle::try_current() {
            task::block_in_place(|| handle.block_on(future))
        } else {
            tokio::runtime::Runtime::new()
                .map_err(|err| SignerError::Custom(format!("failed to create runtime: {err}")))?
                .block_on(future)
        }
    }
}

impl Signer for KmsKeypair {
    fn pubkey(&self) -> Pubkey {
        self.pubkey
    }

    fn try_pubkey(&self) -> Result<Pubkey, SignerError> {
        Ok(self.pubkey)
    }

    fn try_sign_message(&self, message: &[u8]) -> Result<Signature, SignerError> {
        self.block_on_sign(self.sign_message_async(message))
    }

    fn is_interactive(&self) -> bool {
        false
    }
}

fn map_sign_error(err: &SdkError<SignError>) -> SignerError {
    SignerError::Custom(format!("KMS signing failed: {err}"))
}
