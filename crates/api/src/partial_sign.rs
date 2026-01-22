use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use std::cmp::Ordering;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum PartialSignError {
    #[error("signer {0} not found among required signers")]
    SignerNotRequired(Pubkey),

    #[error("too many signature provided: expected {expected}, received {received}")]
    TooManySignatures { expected: usize, received: usize },
}

pub trait PartialSign {
    fn partial_sign<S: Signer>(&mut self, signer: &S) -> Result<Signature, PartialSignError>;
}

impl PartialSign for VersionedTransaction {
    fn partial_sign<S: Signer>(&mut self, signer: &S) -> Result<Signature, PartialSignError> {
        let required_signature_count = self.message.header().num_required_signatures as usize;
        let public_key = signer.pubkey();
        let idx = self
            .message
            .static_account_keys()
            .iter()
            .take(required_signature_count)
            .position(|p| p == &public_key)
            .ok_or(PartialSignError::SignerNotRequired(public_key))?;

        match self.signatures.len().cmp(&required_signature_count) {
            Ordering::Less => {
                self.signatures
                    .resize(required_signature_count, Signature::default());
            }
            Ordering::Greater => {
                if self.signatures[required_signature_count..]
                    .iter()
                    .any(|sig| *sig != Signature::default())
                {
                    return Err(PartialSignError::TooManySignatures {
                        received: self.signatures.len(),
                        expected: required_signature_count,
                    });
                }
                self.signatures.truncate(required_signature_count);
            }
            Ordering::Equal => {}
        }

        let signature = signer.sign_message(&self.message.serialize());
        self.signatures[idx] = signature;

        Ok(signature)
    }
}

#[cfg(test)]
mod tests {
    use super::{PartialSign, PartialSignError};
    use solana_keypair::Keypair;
    use solana_message::{VersionedMessage, legacy::Message};
    use solana_pubkey::Pubkey;
    use solana_signature::Signature;
    use solana_signer::Signer;
    use solana_system_interface::instruction as system_instruction;
    use solana_transaction::versioned::VersionedTransaction;

    #[test]
    fn test_partial_sign_single_signer() {
        let key = Keypair::new();
        let recipient = Pubkey::new_unique();
        let ix = system_instruction::transfer(&key.pubkey(), &recipient, 1);

        // build a legacy message requiring 1 signature
        let msg = VersionedMessage::Legacy(Message::new(&[ix], Some(&key.pubkey())));
        // directly construct with empty signatures
        let mut tx = VersionedTransaction {
            message: msg,
            signatures: Vec::new(),
        };

        let sig = tx.partial_sign(&key).expect("partial sign failed");

        assert_eq!(tx.signatures.len(), 1);
        assert_eq!(tx.signatures[0], sig);
    }

    #[test]
    fn test_partial_sign_multi_signer_empty() {
        let key1 = Keypair::new();
        let key2 = Keypair::new();
        let recipient = Pubkey::new_unique();

        let i1 = system_instruction::transfer(&key1.pubkey(), &recipient, 1);
        let i2 = system_instruction::transfer(&key2.pubkey(), &recipient, 2);

        let msg = VersionedMessage::Legacy(Message::new(&[i1, i2], Some(&key1.pubkey())));
        let mut tx = VersionedTransaction {
            message: msg.clone(),
            signatures: Vec::new(),
        };

        let s1 = tx.partial_sign(&key1).expect("partial sign failed");
        assert_eq!(tx.signatures.len(), 2);
        assert_eq!(tx.signatures[0], s1);
        assert_eq!(tx.signatures[1], Signature::default());

        let s2 = tx.partial_sign(&key2).expect("partial sign failed");
        assert_eq!(tx.signatures.len(), 2);
        assert_eq!(tx.signatures[0], s1);
        assert_eq!(tx.signatures[1], s2);
    }

    #[test]
    fn test_signer_not_required() {
        let key1 = Keypair::new();
        let key2 = Keypair::new();
        let recipient = Pubkey::new_unique();

        let i1 = system_instruction::transfer(&key1.pubkey(), &recipient, 2);

        let msg = VersionedMessage::Legacy(Message::new(&[i1], Some(&key1.pubkey())));
        let mut tx = VersionedTransaction {
            message: msg.clone(),
            signatures: Vec::new(),
        };

        let result = tx.partial_sign(&key2);
        assert!(matches!(
            result,
            Err(PartialSignError::SignerNotRequired(_))
        ));
    }

    #[test]
    fn test_partial_sign_multi_signer_resize_existing() {
        let key1 = Keypair::new();
        let key2 = Keypair::new();
        let recipient = Pubkey::new_unique();

        let i1 = system_instruction::transfer(&key1.pubkey(), &recipient, 1);
        let i2 = system_instruction::transfer(&key2.pubkey(), &recipient, 2);

        let msg = VersionedMessage::Legacy(Message::new(&[i1, i2], Some(&key1.pubkey())));
        // start with a single default signature
        let mut tx = VersionedTransaction {
            message: msg.clone(),
            signatures: vec![Signature::default()],
        };

        let s1 = tx.partial_sign(&key1).expect("partial sign failed");
        assert_eq!(tx.signatures.len(), 2);
        assert_eq!(tx.signatures[0], s1);
        assert_eq!(tx.signatures[1], Signature::default());

        let s2 = tx.partial_sign(&key2).expect("partial sign failed");
        assert_eq!(tx.signatures.len(), 2);
        assert_eq!(tx.signatures[0], s1);
        assert_eq!(tx.signatures[1], s2);
    }

    #[test]
    fn test_partial_sign_multi_signer_too_many() {
        let key1 = Keypair::new();
        let key2 = Keypair::new();
        let recipient = Pubkey::new_unique();

        let i1 = system_instruction::transfer(&key1.pubkey(), &recipient, 1);
        let i2 = system_instruction::transfer(&key2.pubkey(), &recipient, 2);

        let msg = VersionedMessage::Legacy(Message::new(&[i1, i2], Some(&key1.pubkey())));

        // Create a transaction with 3 signatures when only 2 are needed
        // The third signature is non-default to trigger the error
        let mut dummy_sig = [0u8; 64];
        dummy_sig[0] = 1;
        let dummy_sig = Signature::from(dummy_sig);

        let mut tx = VersionedTransaction {
            message: msg.clone(),
            signatures: vec![Signature::default(), Signature::default(), dummy_sig],
        };

        // Attempting to sign should fail because we have a non-default signature in the extra position
        let result = tx.partial_sign(&key1);
        assert!(matches!(
            result,
            Err(PartialSignError::TooManySignatures {
                received: 3,
                expected: 2
            })
        ));
    }
}
