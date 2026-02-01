//! Deterministic derivation of confidential transfer keypairs (ElGamal + AE).
//!
//! Two derivation paths:
//! - `from_signer`: when a full Signer is available (tests, custodial fallback)
//! - `from_signature_bytes`: when only pre-computed signature bytes are available
//!   (non-custodial — client signs a seed and sends the 64-byte signature)
use solana_keypair::Signature;
use solana_pubkey::Pubkey;
use solana_signer::{Signer, SignerError};
use spl_token_2022::solana_zk_sdk::encryption::{auth_encryption::AeKey, elgamal::ElGamalKeypair};

/// Bundled ElGamal and AE keypairs for confidential transfers.
#[derive(Debug, Clone)]
pub struct ConfidentialKeys {
    pub elgamal_keypair: ElGamalKeypair,
    pub ae_key: AeKey,
}

impl ConfidentialKeys {
    /// Derive keys from a Signer and seed.
    pub fn from_signer(signer: &dyn Signer, seed: &[u8]) -> Result<Self, anyhow::Error> {
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
    /// Derive keys from pre-computed signature bytes (non-custodial).
    ///
    /// The client signs a known seed (e.g. ATA address bytes) and sends the
    /// 64-byte signature. The server derives deterministic keypairs from it
    /// without needing the wallet's private key.
    ///
    /// NOTE: Unused today, but may be used in a non-custodial flow in the future.
    pub fn from_signature_bytes(
        wallet_pubkey: Pubkey,
        signature_bytes: &[u8; 64],
        seed: &[u8],
    ) -> Result<Self, anyhow::Error> {
        let signer = PrecomputedSigner {
            pubkey: wallet_pubkey,
            signature: Signature::from(*signature_bytes),
        };
        Self::from_signer(&signer, seed)
    }
}

/// Wraps pre-computed signature bytes as a Signer to satisfy the SDK's
/// `new_from_signer` KDF interface.
struct PrecomputedSigner {
    pubkey: Pubkey,
    signature: Signature,
}

impl Signer for PrecomputedSigner {
    fn pubkey(&self) -> Pubkey {
        self.pubkey
    }

    fn try_pubkey(&self) -> Result<Pubkey, SignerError> {
        Ok(self.pubkey)
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use solana_keypair::Keypair;
    use solana_signer::Signer;
    use spl_token_2022::solana_zk_sdk::encryption::pod::elgamal::PodElGamalPubkey;

    #[test]
    fn test_from_signature_bytes_is_deterministic() {
        let keypair = Keypair::new();
        let seed = b"test_seed";
        let signature = keypair.sign_message(seed);
        let sig_bytes: [u8; 64] = signature.into();

        let keys1 =
            ConfidentialKeys::from_signature_bytes(keypair.pubkey(), &sig_bytes, seed).unwrap();
        let keys2 =
            ConfidentialKeys::from_signature_bytes(keypair.pubkey(), &sig_bytes, seed).unwrap();

        let pod1: PodElGamalPubkey = (*keys1.elgamal_keypair.pubkey()).into();
        let pod2: PodElGamalPubkey = (*keys2.elgamal_keypair.pubkey()).into();
        assert_eq!(pod1, pod2);
    }

    #[test]
    fn test_different_signatures_produce_different_keys() {
        let keypair = Keypair::new();

        let sig1: [u8; 64] = keypair.sign_message(b"seed_one").into();
        let sig2: [u8; 64] = keypair.sign_message(b"seed_two").into();

        let keys1 =
            ConfidentialKeys::from_signature_bytes(keypair.pubkey(), &sig1, b"seed_one").unwrap();
        let keys2 =
            ConfidentialKeys::from_signature_bytes(keypair.pubkey(), &sig2, b"seed_two").unwrap();

        let pod1: PodElGamalPubkey = (*keys1.elgamal_keypair.pubkey()).into();
        let pod2: PodElGamalPubkey = (*keys2.elgamal_keypair.pubkey()).into();
        assert_ne!(pod1, pod2);
    }

    #[test]
    fn test_from_signer_produces_valid_keys() {
        let keypair = Keypair::new();
        let seed = b"test_seed";

        let keys = ConfidentialKeys::from_signer(&keypair, seed).unwrap();
        let _pod: PodElGamalPubkey = (*keys.elgamal_keypair.pubkey()).into();
    }
}
