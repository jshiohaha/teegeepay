use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_instruction::Instruction;
use solana_keypair::{Keypair, Signature};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::{
    extension::{
        BaseStateWithExtensions, StateWithExtensionsOwned,
        confidential_transfer::{
            ConfidentialTransferAccount, ConfidentialTransferMint,
            account_info::TransferAccountInfo,
        },
    },
    solana_zk_sdk::{
        encryption::{elgamal, pod::elgamal::PodElGamalPubkey},
        zk_elgamal_proof_program::{
            self,
            instruction::{ContextStateInfo, close_context_state},
        },
    },
    state::{Account, Mint},
};
use spl_token_client::{
    client::{ProgramRpcClient, ProgramRpcClientSendTransaction},
    token::{ProofAccountWithCiphertext, Token},
};
use spl_token_confidential_transfer_proof_generation::transfer::TransferProofData;
use std::sync::Arc;
use tracing::info;

use crate::solana::balance::{apply_pending_balance, get_confidential_balances};
use crate::solana::deposit::deposit_tokens;
use crate::solana::signature_signer::ConfidentialKeys;
use crate::{handlers::AppError, solana::utils::confidential_keys_for_mint};

struct TransferContext {
    equality_proof_pubkey: Pubkey,
    ciphertext_validity_proof_pubkey: Pubkey,
    range_proof_pubkey: Pubkey,
    ciphertext_validity_proof_account_with_ciphertext: ProofAccountWithCiphertext,
    sender_associated_token_address: Pubkey,
    recipient_associated_token_address: Pubkey,
    sender_transfer_account_info: TransferAccountInfo,
    sender_confidential_keys: ConfidentialKeys,
    recipient_elgamal_pubkey: elgamal::ElGamalPubkey,
    auditor_elgamal_pubkey: elgamal::ElGamalPubkey,
}

async fn ensure_confidential_balance(
    rpc_client: Arc<RpcClient>,
    sender: Arc<dyn Signer + Send + Sync>,
    mint: &Pubkey,
    decimals: u8,
    amount: u64,
) -> Result<()> {
    let (pending_balance, available_balance) =
        get_confidential_balances(rpc_client.clone(), sender.clone(), mint).await?;

    if available_balance >= amount {
        return Ok(());
    }

    if pending_balance == 0 {
        let deposit_amount = amount.saturating_sub(available_balance);
        if deposit_amount == 0 {
            return Ok(());
        }

        let deposit_instructions = deposit_tokens(
            rpc_client.clone(),
            &sender.pubkey(),
            mint,
            decimals,
            deposit_amount,
        )
        .await?;
        let mut deposit_signers: Vec<&dyn Signer> = deposit_instructions
            .additional_signers
            .iter()
            .map(|signer| signer.as_ref() as &dyn Signer)
            .collect();
        if deposit_signers.is_empty() {
            deposit_signers.push(sender.as_ref());
        }

        let deposit_tx = Transaction::new_signed_with_payer(
            &deposit_instructions.instructions,
            Some(&sender.pubkey()),
            &deposit_signers,
            rpc_client.get_latest_blockhash().await?,
        );
        let deposit_signature = rpc_client.send_and_confirm_transaction(&deposit_tx).await?;
        info!(
            "Transfer [Deposit Confidential Pending Balance] with signature={:?}",
            deposit_signature
        );
    }

    let apply_instructions = apply_pending_balance(
        rpc_client.clone(),
        sender.clone(),
        sender.clone(),
        mint,
        decimals,
    )
    .await?;
    let mut apply_signers: Vec<&dyn Signer> = apply_instructions
        .additional_signers
        .iter()
        .map(|signer| signer.as_ref() as &dyn Signer)
        .collect();
    if apply_signers.is_empty() {
        apply_signers.push(sender.as_ref());
    }

    let apply_tx = Transaction::new_signed_with_payer(
        &apply_instructions.instructions,
        Some(&sender.pubkey()),
        &apply_signers,
        rpc_client.get_latest_blockhash().await?,
    );
    let apply_signature = rpc_client.send_and_confirm_transaction(&apply_tx).await?;
    info!(
        "Transfer [Apply Pending Balance] with signature={:?}",
        apply_signature
    );

    let (pending_after, available_after) =
        get_confidential_balances(rpc_client, sender, mint).await?;
    if available_after >= amount {
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "Insufficient confidential balance after applying pending. available={}, pending={}, amount={}",
        available_after,
        pending_after,
        amount
    ))
}

pub async fn with_split_proofs(
    rpc_client: Arc<RpcClient>,
    sender: Arc<Keypair>,
    recipient: &Pubkey,
    confidential_transfer_amount: u64,
    mint: &Pubkey,
    decimals: u8,
) -> Result<Vec<Signature>> {
    // TODO: add these transactions?
    ensure_confidential_balance(
        rpc_client.clone(),
        sender.clone(),
        mint,
        decimals,
        confidential_transfer_amount,
    )
    .await?;

    let (transactions, ctx) = prepare_proof_transactions(
        rpc_client.clone(),
        sender.clone(),
        recipient,
        confidential_transfer_amount,
        mint,
        decimals,
    )
    .await
    .map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!(
            "Failed to prepare proof transactions: {:?}",
            e
        ))
    })?;

    let signature_1 = rpc_client
        .send_and_confirm_transaction(&transactions[0])
        .await?;
    info!(
        "Transfer [Allocate Proof Accounts] with signature={:?}",
        signature_1
    );

    let signature_2 = rpc_client
        .send_and_confirm_transaction(&transactions[1])
        .await?;
    info!(
        "Transfer [Encode Range Proof] with signature={:?}",
        signature_2
    );

    let signature_3 = rpc_client
        .send_and_confirm_transaction(&transactions[2])
        .await?;
    info!(
        "Transfer [Encode Remaining Proofs] with signature={:?}",
        signature_3
    );

    let transfer_signature = execute_transfer(
        rpc_client.clone(),
        sender.clone(),
        &ctx,
        confidential_transfer_amount,
        mint,
        decimals,
    )
    .await
    .map_err(|e| {
        AppError::internal_server_error(anyhow::anyhow!("Failed to execute transfer: {:?}", e))
    })?;

    let close_ixs = build_close_proof_accounts_ixs(rpc_client.clone(), sender.clone(), &ctx)?;
    let close_tx = Transaction::new_signed_with_payer(
        &close_ixs,
        Some(&sender.pubkey()),
        &[&sender],
        rpc_client.clone().get_latest_blockhash().await?,
    );
    let signature_4 = rpc_client
        .clone()
        .send_and_confirm_transaction(&close_tx)
        .await?;
    info!(
        "Transfer [Close Proof Accounts] with signature={:?}",
        signature_4
    );

    Ok(vec![
        signature_1,
        signature_2,
        signature_3,
        transfer_signature,
        signature_4,
    ])
}

async fn execute_transfer(
    rpc_client: Arc<RpcClient>,
    sender: Arc<dyn Signer + Send + Sync>,
    ctx: &TransferContext,
    confidential_transfer_amount: u64,
    mint: &Pubkey,
    decimals: u8,
) -> Result<Signature> {
    let token = {
        let program_client: ProgramRpcClient<ProgramRpcClientSendTransaction> =
            ProgramRpcClient::new(rpc_client, ProgramRpcClientSendTransaction);
        Token::new(
            Arc::new(program_client),
            &spl_token_2022::id(),
            mint,
            Some(decimals),
            sender.clone(),
        )
    };

    let response = token
        .confidential_transfer_transfer(
            &ctx.sender_associated_token_address,
            &ctx.recipient_associated_token_address,
            &sender.pubkey(),
            Some(&ctx.equality_proof_pubkey),
            Some(&ctx.ciphertext_validity_proof_account_with_ciphertext),
            Some(&ctx.range_proof_pubkey),
            confidential_transfer_amount,
            Some(ctx.sender_transfer_account_info.clone()),
            &ctx.sender_confidential_keys.elgamal_keypair,
            &ctx.sender_confidential_keys.ae_key,
            &ctx.recipient_elgamal_pubkey,
            Some(&ctx.auditor_elgamal_pubkey),
            &[&sender],
        )
        .await?;

    match response {
        spl_token_client::client::RpcClientResponse::Signature(sig) => Ok(sig),
        _ => Err(anyhow::anyhow!("Expected signature response from transfer")),
    }
}

fn build_close_proof_accounts_ixs(
    _rpc_client: Arc<RpcClient>,
    sender: Arc<dyn Signer + Send + Sync>,
    ctx: &TransferContext,
) -> Result<Vec<Instruction>> {
    let context_state_authority_pubkey = sender.pubkey();
    let destination_account = &sender.pubkey();

    let close_equality_proof_instruction = close_context_state(
        ContextStateInfo {
            context_state_account: &ctx.equality_proof_pubkey,
            context_state_authority: &context_state_authority_pubkey,
        },
        &destination_account,
    );

    let close_ciphertext_validity_proof_instruction = close_context_state(
        ContextStateInfo {
            context_state_account: &ctx.ciphertext_validity_proof_pubkey,
            context_state_authority: &context_state_authority_pubkey,
        },
        &destination_account,
    );

    let close_range_proof_instruction = close_context_state(
        ContextStateInfo {
            context_state_account: &ctx.range_proof_pubkey,
            context_state_authority: &context_state_authority_pubkey,
        },
        &destination_account,
    );

    // let recent_blockhash = client.get_latest_blockhash()?;
    // let tx = Transaction::new_signed_with_payer(
    //     &[
    //         close_equality_proof_instruction,
    //         close_ciphertext_validity_proof_instruction,
    //         close_range_proof_instruction,
    //     ],
    //     Some(&sender.pubkey()),
    //     &[&sender],
    //     recent_blockhash,
    // );

    Ok(vec![
        close_equality_proof_instruction,
        close_ciphertext_validity_proof_instruction,
        close_range_proof_instruction,
    ])
}

/// Prepare proof transactions using pre-derived confidential keys.
///
/// # Arguments
/// * `rpc_client` - RPC client
/// * `sender` - Sender signer (for transaction signing)
/// * `recipient` - Recipient public key
/// * `confidential_transfer_amount` - Amount to transfer
/// * `mint` - Mint address
/// * `decimals` - Token decimals
/// * `sender_confidential_keys` - Pre-derived ElGamal and AE keypairs for the sender
async fn prepare_proof_transactions_with_keys(
    rpc_client: Arc<RpcClient>,
    sender: Arc<dyn Signer + Send + Sync>,
    recipient: &Pubkey,
    confidential_transfer_amount: u64,
    mint: &Pubkey,
    decimals: u8,
    sender_confidential_keys: &ConfidentialKeys,
) -> Result<(Vec<Transaction>, TransferContext)> {
    let sender_associated_token_address: Pubkey =
        get_associated_token_address_with_program_id(&sender.pubkey(), mint, &spl_token_2022::id());

    let token = {
        let program_client: ProgramRpcClient<ProgramRpcClientSendTransaction> =
            ProgramRpcClient::new(rpc_client.clone(), ProgramRpcClientSendTransaction);

        Token::new(
            Arc::new(program_client),
            &spl_token_2022::id(),
            mint,
            Some(decimals),
            sender.clone(),
        )
    };
    let recipient_associated_token_address =
        get_associated_token_address_with_program_id(recipient, mint, &spl_token_2022::id());

    let context_state_authority = &sender;

    let equality_proof_context_state_account = Keypair::new();
    let equality_proof_pubkey = equality_proof_context_state_account.pubkey();

    let ciphertext_validity_proof_context_state_account = Keypair::new();
    let ciphertext_validity_proof_pubkey = ciphertext_validity_proof_context_state_account.pubkey();

    let range_proof_context_state_account = Keypair::new();
    let range_proof_pubkey = range_proof_context_state_account.pubkey();

    let sender_token_account_info = token
        .get_account_info(&sender_associated_token_address)
        .await?;

    let sender_account_extension_data =
        sender_token_account_info.get_extension::<ConfidentialTransferAccount>()?;

    let sender_transfer_account_info = TransferAccountInfo::new(sender_account_extension_data);

    let recipient_account = token
        .get_account(recipient_associated_token_address)
        .await?;

    let recipient_elgamal_pubkey: elgamal::ElGamalPubkey =
        StateWithExtensionsOwned::<Account>::unpack(recipient_account.data)?
            .get_extension::<ConfidentialTransferAccount>()?
            .elgamal_pubkey
            .try_into()?;

    let mint_account = token.get_account(*mint).await?;

    let auditor_elgamal_pubkey_option = Option::<PodElGamalPubkey>::from(
        StateWithExtensionsOwned::<Mint>::unpack(mint_account.data)?
            .get_extension::<ConfidentialTransferMint>()?
            .auditor_elgamal_pubkey,
    );

    let auditor_elgamal_pubkey: elgamal::ElGamalPubkey = auditor_elgamal_pubkey_option
        .ok_or(anyhow::anyhow!("No Auditor ElGamal pubkey"))?
        .try_into()?;

    let TransferProofData {
        equality_proof_data,
        ciphertext_validity_proof_data_with_ciphertext,
        range_proof_data,
    } = sender_transfer_account_info.generate_split_transfer_proof_data(
        confidential_transfer_amount,
        &sender_confidential_keys.elgamal_keypair,
        &sender_confidential_keys.ae_key,
        &recipient_elgamal_pubkey,
        Some(&auditor_elgamal_pubkey),
    )?;

    let (range_create_ix, range_verify_ix) =
        get_zk_proof_context_state_account_creation_instructions(
            rpc_client.clone(),
            &sender.pubkey(),
            &range_proof_context_state_account.pubkey(),
            &context_state_authority.pubkey(),
            &range_proof_data,
        )
        .await?;

    let (equality_create_ix, equality_verify_ix) =
        get_zk_proof_context_state_account_creation_instructions(
            rpc_client.clone(),
            &sender.pubkey(),
            &equality_proof_context_state_account.pubkey(),
            &context_state_authority.pubkey(),
            &equality_proof_data,
        )
        .await?;

    let (cv_create_ix, cv_verify_ix) = get_zk_proof_context_state_account_creation_instructions(
        rpc_client.clone(),
        &sender.pubkey(),
        &ciphertext_validity_proof_context_state_account.pubkey(),
        &context_state_authority.pubkey(),
        &ciphertext_validity_proof_data_with_ciphertext.proof_data,
    )
    .await?;

    let tx1 = Transaction::new_signed_with_payer(
        &[
            range_create_ix.clone(),
            equality_create_ix.clone(),
            cv_create_ix.clone(),
        ],
        Some(&sender.pubkey()),
        &[
            &sender,
            &range_proof_context_state_account as &dyn Signer,
            &equality_proof_context_state_account as &dyn Signer,
            &ciphertext_validity_proof_context_state_account as &dyn Signer,
        ],
        rpc_client.get_latest_blockhash().await?,
    );

    let tx2 = Transaction::new_signed_with_payer(
        &[range_verify_ix],
        Some(&sender.pubkey()),
        &[&sender],
        rpc_client.get_latest_blockhash().await?,
    );

    let tx3 = Transaction::new_signed_with_payer(
        &[equality_verify_ix, cv_verify_ix],
        Some(&sender.pubkey()),
        &[&sender],
        rpc_client.get_latest_blockhash().await?,
    );

    let ciphertext_validity_proof_account_with_ciphertext = ProofAccountWithCiphertext {
        context_state_account: ciphertext_validity_proof_pubkey,
        ciphertext_lo: ciphertext_validity_proof_data_with_ciphertext.ciphertext_lo,
        ciphertext_hi: ciphertext_validity_proof_data_with_ciphertext.ciphertext_hi,
    };

    let ctx = TransferContext {
        equality_proof_pubkey,
        ciphertext_validity_proof_pubkey,
        range_proof_pubkey,
        ciphertext_validity_proof_account_with_ciphertext,
        sender_associated_token_address,
        recipient_associated_token_address,
        sender_transfer_account_info,
        sender_confidential_keys: sender_confidential_keys.clone(),
        recipient_elgamal_pubkey,
        auditor_elgamal_pubkey,
    };

    Ok((vec![tx1, tx2, tx3], ctx))
}

/// Prepare proof transactions - derives confidential keys from the sender signer.
async fn prepare_proof_transactions(
    rpc_client: Arc<RpcClient>,
    sender: Arc<dyn Signer + Send + Sync>,
    recipient: &Pubkey,
    confidential_transfer_amount: u64,
    mint: &Pubkey,
    decimals: u8,
) -> Result<(Vec<Transaction>, TransferContext)> {
    let sender_confidential_keys = confidential_keys_for_mint(sender.clone(), mint)?;
    prepare_proof_transactions_with_keys(
        rpc_client,
        sender,
        recipient,
        confidential_transfer_amount,
        mint,
        decimals,
        &sender_confidential_keys,
    )
    .await
}

async fn get_zk_proof_context_state_account_creation_instructions<
    ZK: bytemuck::Pod + zk_elgamal_proof_program::proof_data::ZkProofData<U>,
    U: bytemuck::Pod,
>(
    rpc_client: Arc<RpcClient>,
    fee_payer_pubkey: &Pubkey,
    context_state_account_pubkey: &Pubkey,
    context_state_authority_pubkey: &Pubkey,
    proof_data: &ZK,
) -> Result<(Instruction, Instruction)> {
    use spl_token_confidential_transfer_proof_extraction::instruction::zk_proof_type_to_instruction;
    use std::mem::size_of;

    let space = size_of::<zk_elgamal_proof_program::state::ProofContextState<U>>();
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
