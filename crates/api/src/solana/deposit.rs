//! Deposit tokens into the confidential (pending) balance.
//!
//! Moves tokens from a token account's non-confidential balance into its
//! confidential pending balance via the SPL Token-2022 confidential transfer
//! `deposit` instruction. The pending balance must later be applied (see
//! [`crate::solana::balance::apply_pending_balance`]) before the funds
//! become available for confidential transfers.

use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::extension::confidential_transfer::instruction::deposit;
use std::sync::Arc;

use crate::solana::GeneratedInstructions;

/// Deposit tokens from non-confidential balance to "pending" balance
pub async fn deposit_tokens(
    _rpc_client: Arc<RpcClient>,
    depositor: &Pubkey,
    mint: &Pubkey,
    decimals: u8,
    amount: u64,
) -> Result<GeneratedInstructions> {
    let depositor_token_account =
        get_associated_token_address_with_program_id(depositor, &mint, &spl_token_2022::id());

    // deposit from non-confidential balance to "pending" balance
    let deposit_instruction = deposit(
        &spl_token_2022::id(),
        &depositor_token_account,
        &mint,
        amount,
        decimals,
        depositor,
        &[],
    )?;

    Ok(GeneratedInstructions {
        instructions: vec![deposit_instruction],
        additional_signers: vec![],
    })
}
