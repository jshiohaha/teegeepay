use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::extension::confidential_transfer::instruction::deposit;
use std::sync::Arc;

use crate::solana::GeneratedInstructions;

pub async fn deposit_tokens(
    _rpc_client: Arc<RpcClient>,
    depositor: &Pubkey,
    mint: &Pubkey,
    decimals: u8,
    amount: u64,
) -> Result<GeneratedInstructions> {
    // Confidential balance has separate "pending" and "available" balances
    // Must first deposit tokens from non-confidential balance to  "pending" confidential balance

    let depositor_token_account = get_associated_token_address_with_program_id(
        depositor, // Token account owner
        &mint,     // Mint
        &spl_token_2022::id(),
    );

    // Instruction to deposit from non-confidential balance to "pending" balance
    let deposit_instruction = deposit(
        &spl_token_2022::id(),
        &depositor_token_account, // Token account
        &mint,                    // Mint
        amount,                   // Amount to deposit
        decimals,                 // Mint decimals
        depositor,                // Token account owner
        &[],                      // Signers
    )?;

    Ok(GeneratedInstructions {
        instructions: vec![deposit_instruction],
        additional_signers: vec![], // depositor.clone()
    })
}
