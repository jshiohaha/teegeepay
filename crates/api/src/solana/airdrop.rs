use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::CommitmentConfig;
use solana_keypair::Signature;
use solana_pubkey::Pubkey;
use std::sync::Arc;

pub async fn request_and_confirm(
    rpc_client: Arc<RpcClient>,
    pubkey: &Pubkey,
    lamport_amount: u64,
) -> Result<Signature> {
    let signature = rpc_client
        .request_airdrop(pubkey, lamport_amount)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to request airdrop: {:?}", e))?;
    rpc_client
        .confirm_transaction_with_commitment(&signature, CommitmentConfig::confirmed())
        .await?;
    Ok(signature)
}
