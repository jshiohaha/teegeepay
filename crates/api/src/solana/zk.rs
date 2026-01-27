use {
    anyhow::Result,
    bytemuck::Pod,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_instruction::Instruction,
    solana_pubkey::Pubkey,
    spl_token_2022::solana_zk_sdk::zk_elgamal_proof_program::{
        self, instruction::ContextStateInfo, proof_data::ZkProofData, state::ProofContextState,
    },
    spl_token_confidential_transfer_proof_extraction::instruction::zk_proof_type_to_instruction,
    std::{mem::size_of, sync::Arc},
};

/// Build create + verify instructions for a proof context state account.
pub async fn get_zk_proof_context_state_account_creation_instructions<
    ZK: Pod + ZkProofData<U>,
    U: Pod,
>(
    rpc_client: Arc<RpcClient>,
    fee_payer_pubkey: &Pubkey,
    context_state_account_pubkey: &Pubkey,
    context_state_authority_pubkey: &Pubkey,
    proof_data: &ZK,
) -> Result<(Instruction, Instruction)> {
    let space = size_of::<ProofContextState<U>>();
    let rent = rpc_client
        .get_minimum_balance_for_rent_exemption(space)
        .await
        .map_err(|_| anyhow::anyhow!("Failed to get minimum balance for rent exemption"))?;

    let context_state_info = ContextStateInfo {
        context_state_account: context_state_account_pubkey,
        context_state_authority: context_state_authority_pubkey,
    };

    let instruction_type = zk_proof_type_to_instruction(ZK::PROOF_TYPE)?;

    let create_account_ix = solana_system_interface::instruction::create_account(
        fee_payer_pubkey,
        context_state_account_pubkey,
        rent,
        space as u64,
        &zk_elgamal_proof_program::id(),
    );

    let verify_proof_ix =
        instruction_type.encode_verify_proof(Some(context_state_info), proof_data);

    Ok((create_account_ix, verify_proof_ix))
}
