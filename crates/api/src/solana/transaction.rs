use crate::partial_sign::PartialSign;
use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_hash::Hash;
use solana_instruction::Instruction;
use solana_message::{VersionedMessage, v0::Message};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use std::sync::Arc;
use tracing::error;

pub async fn build_transaction(
    rpc_client: Arc<RpcClient>,
    recent_blockhash: Option<Hash>,
    instructions: Vec<Instruction>,
    fee_payer: Arc<dyn Signer + Send + Sync>,
    additional_signers: Vec<Arc<dyn Signer + Send + Sync>>,
) -> Result<VersionedTransaction> {
    let mut transaction = build_transaction_with_signers(
        rpc_client,
        recent_blockhash,
        instructions,
        fee_payer.clone(),
    )
    .await?;

    for signer in additional_signers {
        match transaction.partial_sign(&signer) {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to partial sign transaction: {:?}", e);
            }
        }
    }

    Ok(transaction)
}

pub async fn build_transaction_with_signers(
    rpc_client: Arc<RpcClient>,
    recent_blockhash: Option<Hash>,
    instructions: Vec<Instruction>,
    fee_payer: Arc<dyn Signer + Send + Sync>,
) -> Result<VersionedTransaction> {
    let recent_blockhash = match recent_blockhash {
        Some(hash) => hash,
        None => rpc_client.get_latest_blockhash().await?,
    };

    // TODO: add compute unit ix's?
    let mut transaction: VersionedTransaction = VersionedTransaction {
        signatures: vec![],
        message: VersionedMessage::V0(Message::try_compile(
            &fee_payer.pubkey(),
            &instructions,
            &[],
            recent_blockhash,
        )?),
    };

    transaction.partial_sign(&fee_payer)?;

    Ok(transaction)
}
