pub mod airdrop;
pub mod balance;
pub mod create;
pub mod deposit;
pub mod mint;
pub mod signature_signer;
pub mod supply;
pub mod tokens;
pub mod transaction;
pub mod transfer;
pub mod utils;
pub mod withdraw;
pub mod zk;

use solana_instruction::Instruction;
use solana_signer::Signer;
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct GeneratedInstructions {
    pub instructions: Vec<Instruction>,
    pub additional_signers: Vec<Arc<dyn Signer + Send + Sync>>,
}
