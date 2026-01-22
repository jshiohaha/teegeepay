//! SignatureSigner - A minimal adapter for browser wallet support
//!
//! Browser wallets (Phantom, Solflare, Backpack) cannot export private keys,
//! but they can sign messages. This module provides a way to derive ElGamal
//! and AE keypairs using Solana's built-in KDF by wrapping a pre-computed
//! signature in a Signer trait implementation.
//!
//! # Flow for browser wallet support:
//! 1. Client requests the public seed (e.g., token account address bytes)
//! 2. Browser wallet signs the public seed
//! 3. Client sends signature bytes + wallet pubkey to the API
//! 4. API wraps them in SignatureSigner
//! 5. API uses SignatureSigner to derive ElGamal/AE keys via Solana's KDF
//!
//! # Example Client-Side (TypeScript)
//! ```typescript
//! // 1. Get the ATA address (the seed)
//! const ata = getAssociatedTokenAddressSync(mint, walletPublicKey, false, TOKEN_2022_PROGRAM_ID);
//!
//! // 2. Sign the ATA address bytes with the browser wallet
//! const signature = await wallet.signMessage(ata.toBytes());
//!
//! // 3. Send to API
//! const response = await fetch('/api/wallets/setup-token-account', {
//!   method: 'POST',
//!   body: JSON.stringify({
//!     walletPubkey: walletPublicKey.toBase58(),
//!     signature: Array.from(signature), // 64 bytes as array
//!     mint: mint.toBase58(),
//!   }),
//! });
//! ```

use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use solana_keypair::Signature;
use solana_pubkey::Pubkey;
use solana_signer::{Signer, SignerError};
use spl_token_2022::solana_zk_sdk::encryption::{auth_encryption::AeKey, elgamal::ElGamalKeypair};

/// A minimal signer that wraps a pre-computed signature from a browser wallet.
///
/// This allows reusing Solana's KDF for deriving ElGamal and AE keypairs
/// without requiring access to the wallet's private key.
#[derive(Debug, Clone)]
pub struct SignatureSigner {
    pubkey: Pubkey,
    signature: Signature,
}

impl SignatureSigner {
    /// Create a new SignatureSigner from a wallet pubkey and pre-computed signature.
    ///
    /// # Arguments
    /// * `pubkey` - The wallet's public key
    /// * `signature` - The signature of the public seed (e.g., ATA address bytes)
    pub fn new(pubkey: Pubkey, signature: Signature) -> Self {
        Self { pubkey, signature }
    }

    /// Create from raw signature bytes (64 bytes)
    pub fn from_bytes(pubkey: Pubkey, signature_bytes: &[u8; 64]) -> Self {
        Self {
            pubkey,
            signature: Signature::from(*signature_bytes),
        }
    }
}

impl Signer for SignatureSigner {
    fn pubkey(&self) -> Pubkey {
        self.pubkey
    }

    fn try_pubkey(&self) -> Result<Pubkey, SignerError> {
        Ok(self.pubkey)
    }

    /// Returns the pre-computed signature.
    ///
    /// NOTE: This ignores the message parameter since the signature was
    /// pre-computed by the browser wallet for a specific message (the public seed).
    fn sign_message(&self, _message: &[u8]) -> Signature {
        self.signature
    }

    fn try_sign_message(&self, _message: &[u8]) -> Result<Signature, SignerError> {
        Ok(self.signature)
    }

    fn is_interactive(&self) -> bool {
        false
    }
}

/// Holds the derived confidential transfer keypairs
#[derive(Debug, Clone)]
pub struct ConfidentialKeys {
    pub elgamal_keypair: ElGamalKeypair,
    pub ae_key: AeKey,
}

impl ConfidentialKeys {
    /// Derive confidential keys from a Signer and public seed.
    ///
    /// This is the standard way to derive keys when you have a full Signer
    /// (e.g., a Keypair stored server-side).
    pub fn from_signer<S: Signer>(signer: &S, seed: &[u8]) -> Result<Self, anyhow::Error> {
        let elgamal_keypair = ElGamalKeypair::new_from_signer(signer, seed)
            .map_err(|e| anyhow::anyhow!("Failed to create ElGamal keypair: {}", e))?;
        let ae_key = AeKey::new_from_signer(signer, seed)
            .map_err(|e| anyhow::anyhow!("Failed to create AE key: {}", e))?;
        Ok(Self {
            elgamal_keypair,
            ae_key,
        })
    }

    #[allow(dead_code)]
    /// Derive confidential keys from a pre-computed signature.
    ///
    /// This is the browser wallet flow:
    /// 1. The browser wallet signs the `seed` (typically the ATA address bytes)
    /// 2. The signature + wallet pubkey are sent to the API
    /// 3. We wrap them in a SignatureSigner and use Solana's KDF
    ///
    /// # Arguments
    /// * `wallet_pubkey` - The wallet's public key
    /// * `signature` - The signature of the seed, produced by the browser wallet
    /// * `seed` - The public seed that was signed (e.g., ATA address bytes)
    pub fn from_signature(
        wallet_pubkey: Pubkey,
        signature: Signature,
        seed: &[u8],
    ) -> Result<Self, anyhow::Error> {
        let signer = SignatureSigner::new(wallet_pubkey, signature);
        Self::from_signer(&signer, seed)
    }

    /// Derive confidential keys from raw signature bytes.
    ///
    /// Convenience method for API handlers that receive signature as bytes.
    pub fn from_signature_bytes(
        wallet_pubkey: Pubkey,
        signature_bytes: &[u8; 64],
        seed: &[u8],
    ) -> Result<Self, anyhow::Error> {
        let signer = SignatureSigner::from_bytes(wallet_pubkey, signature_bytes);
        Self::from_signer(&signer, seed)
    }
}

/// Input for deriving confidential keys from a browser wallet signature.
///
/// Used in API request payloads. The signature should be produced by the
/// browser wallet signing the public seed (typically the ATA address bytes).
///
/// # Example Request Payload
/// ```json
/// {
///   "walletPubkey": "5Zzguz4NsSRFxGkHfM4FmsFpGZiCDtY72zH2jzMcqkJx",
///   "signatureBytes": [1, 2, 3, ..., 64]  // 64 bytes as JSON array
/// }
/// ```
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureKeyDerivation {
    /// The wallet's public key (base58 encoded)
    #[serde_as(as = "DisplayFromStr")]
    pub wallet_pubkey: Pubkey,
    /// The signature of the seed (64 bytes as JSON array)
    #[serde_as(as = "serde_with::Bytes")]
    pub signature_bytes: Vec<u8>,
}

impl SignatureKeyDerivation {
    pub fn new(wallet_pubkey: Pubkey, signature_bytes: Vec<u8>) -> Self {
        Self {
            wallet_pubkey,
            signature_bytes,
        }
    }

    /// Derive confidential keys using this signature derivation info and a seed.
    ///
    /// The seed should be the same bytes that were signed by the browser wallet
    /// (typically the ATA address bytes).
    pub fn derive_keys(&self, seed: &[u8]) -> Result<ConfidentialKeys, anyhow::Error> {
        if self.signature_bytes.len() != 64 {
            return Err(anyhow::anyhow!(
                "Signature must be exactly 64 bytes, got {}",
                self.signature_bytes.len()
            ));
        }
        let signature_array: [u8; 64] = self
            .signature_bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("Failed to convert signature to array"))?;
        ConfidentialKeys::from_signature_bytes(self.wallet_pubkey, &signature_array, seed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_keypair::Keypair;
    use spl_token_2022::solana_zk_sdk::encryption::pod::elgamal::PodElGamalPubkey;

    #[test]
    fn test_signature_signer_produces_deterministic_keys() {
        // The SignatureSigner approach produces DIFFERENT keys than direct signer,
        // but the keys are DETERMINISTIC - same signature always produces same keys.
        //
        // This is the correct behavior because:
        // 1. Browser wallet signs the seed directly
        // 2. SignatureSigner returns that signature when new_from_signer calls sign_message
        // 3. The key derivation uses those signature bytes deterministically

        let keypair = Keypair::new();
        let seed = b"test_seed_for_ata_address";

        // Simulate browser wallet signing the seed
        let signature = keypair.sign_message(seed);

        // Derive keys twice using SignatureSigner - should be identical
        let keys1 = ConfidentialKeys::from_signature(keypair.pubkey(), signature, seed).unwrap();
        let keys2 = ConfidentialKeys::from_signature(keypair.pubkey(), signature, seed).unwrap();

        // Both derivations should produce identical keys
        let pod1: PodElGamalPubkey = (*keys1.elgamal_keypair.pubkey()).into();
        let pod2: PodElGamalPubkey = (*keys2.elgamal_keypair.pubkey()).into();
        assert_eq!(
            pod1, pod2,
            "SignatureSigner should produce deterministic keys"
        );
    }

    #[test]
    fn test_different_signatures_produce_different_keys() {
        // Different signatures (from different seeds) should produce different keys
        let keypair = Keypair::new();

        let seed1 = b"seed_one";
        let seed2 = b"seed_two";

        let signature1 = keypair.sign_message(seed1);
        let signature2 = keypair.sign_message(seed2);

        let keys1 = ConfidentialKeys::from_signature(keypair.pubkey(), signature1, seed1).unwrap();
        let keys2 = ConfidentialKeys::from_signature(keypair.pubkey(), signature2, seed2).unwrap();

        let pod1: PodElGamalPubkey = (*keys1.elgamal_keypair.pubkey()).into();
        let pod2: PodElGamalPubkey = (*keys2.elgamal_keypair.pubkey()).into();
        assert_ne!(
            pod1, pod2,
            "Different signatures should produce different keys"
        );
    }

    #[test]
    fn test_signature_signer_from_bytes() {
        let keypair = Keypair::new();
        let seed = b"test_seed";

        let signature = keypair.sign_message(seed);
        let signature_bytes: [u8; 64] = signature.into();

        let keys = ConfidentialKeys::from_signature_bytes(keypair.pubkey(), &signature_bytes, seed)
            .unwrap();

        // Should not panic and should produce valid keys
        // The fact that we can create a Pod type means the key is valid
        let _pod: PodElGamalPubkey = (*keys.elgamal_keypair.pubkey()).into();
    }

    #[test]
    fn test_signature_key_derivation_struct() {
        let keypair = Keypair::new();
        let seed = b"test_seed";

        let signature = keypair.sign_message(seed);
        let signature_bytes: [u8; 64] = signature.into();

        // Test the SignatureKeyDerivation struct (used in API payloads)
        let derivation = SignatureKeyDerivation::new(keypair.pubkey(), signature_bytes.to_vec());
        let keys = derivation.derive_keys(seed).unwrap();

        // Verify we get valid keys
        let _pod: PodElGamalPubkey = (*keys.elgamal_keypair.pubkey()).into();
    }

    #[test]
    fn test_signature_key_derivation_rejects_wrong_length() {
        let keypair = Keypair::new();
        let seed = b"test_seed";

        // Wrong length signature (should be 64 bytes)
        let wrong_signature = vec![0u8; 32];

        let derivation = SignatureKeyDerivation::new(keypair.pubkey(), wrong_signature);
        let result = derivation.derive_keys(seed);

        assert!(result.is_err(), "Should reject signature with wrong length");
    }
}
