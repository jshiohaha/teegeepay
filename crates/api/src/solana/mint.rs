use crate::solana::GeneratedInstructions;
use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::instruction::mint_to;
use std::sync::Arc;

pub async fn go(
    _rpc_client: Arc<RpcClient>,
    _funding_address: &Pubkey,
    mint_authority: Arc<dyn Signer + Send + Sync>,
    token_account_owner: &Pubkey,
    mint: &Pubkey,
    mint_amount: u64,
) -> Result<GeneratedInstructions> {
    let receiving_token_account = get_associated_token_address_with_program_id(
        &token_account_owner, // Token account owner
        mint,                 // Mint
        &spl_token_2022::id(),
    );

    // Instruction to mint tokens
    let mint_to_instruction: Instruction = mint_to(
        &spl_token_2022::id(),
        mint,                        // Mint
        &receiving_token_account,    // Token account to mint to
        &mint_authority.pubkey(),    // Token account owner
        &[&mint_authority.pubkey()], // Additional signers (mint authority)
        mint_amount,                 // Amount to mint
    )?;

    Ok(GeneratedInstructions {
        instructions: vec![mint_to_instruction],
        additional_signers: vec![mint_authority],
    })
}
