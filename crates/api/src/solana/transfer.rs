//! Confidential token transfers using SPL Token-2022 split proofs.
//!
//! Orchestrates the full confidential transfer flow: ensuring the sender
//! has sufficient confidential balance (depositing and applying pending
//! balance if needed), generating ZK proof context accounts (equality,
//! ciphertext-validity, and range proofs) across multiple transactions,
//! executing the confidential transfer, and closing proof accounts to
//! reclaim rent.

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
        zk_elgamal_proof_program::instruction::{ContextStateInfo, close_context_state},
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
use crate::solana::confidential_keys::ConfidentialKeys;
use crate::solana::deposit::deposit_tokens;
use crate::solana::zk::get_zk_proof_context_state_account_creation_instructions;
use crate::{handlers::AppError, solana::utils::confidential_keys_for_mint};

/// State carried between the proof-setup phase and the transfer/cleanup phases.
///
/// After `execute_proof_transactions_with_keys` creates the three proof context
/// state accounts on-chain, this struct bundles everything the subsequent
/// `execute_transfer` and `build_close_proof_accounts_ixs` calls need:
///   - Pubkeys of the three proof accounts (so the transfer instruction can
///     reference them, and the close instructions can reclaim their rent).
///   - The split ciphertexts (lo/hi) attached to the ciphertext validity proof.
///   - Sender/recipient addressing and cryptographic keys.
struct TransferContext {
    /// On-chain account holding the verified equality proof.
    equality_proof_pubkey: Pubkey,
    /// On-chain account holding the verified ciphertext validity proof.
    ciphertext_validity_proof_pubkey: Pubkey,
    /// On-chain account holding the verified range proof (Bulletproofs).
    range_proof_pubkey: Pubkey,
    /// Ciphertext validity proof account bundled with the split ciphertexts
    /// (`ciphertext_lo` and `ciphertext_hi`) that encode the transfer amount.
    ciphertext_validity_proof_account_with_ciphertext: ProofAccountWithCiphertext,
    sender_associated_token_address: Pubkey,
    recipient: Pubkey,
    recipient_associated_token_address: Pubkey,
    /// Snapshot of the sender's confidential transfer extension state, used to
    /// provide the encrypted available balance to the transfer instruction.
    sender_transfer_account_info: TransferAccountInfo,
    /// Sender's ElGamal keypair and AE key, needed to decrypt/re-encrypt balances.
    sender_confidential_keys: ConfidentialKeys,
    /// Recipient's ElGamal public key -- transfer ciphertexts are encrypted under
    /// this key so only the recipient can decrypt the received amount.
    recipient_elgamal_pubkey: elgamal::ElGamalPubkey,
    /// Optional auditor ElGamal public key from the mint. When set, transfer
    /// ciphertexts are also encrypted under this key for compliance auditing.
    auditor_elgamal_pubkey: elgamal::ElGamalPubkey,
}

// entrypoint for the confidential transfer process
pub async fn invoke_confidential_transfer(
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

    let (transactions, ctx) = execute_proof_transactions(
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

    // 3 proof setup TXs, executed sequentially
    // followed by, transfer
    // followed by, close proof accounts
    let signature_1 = rpc_client
        .send_and_confirm_transaction(&transactions[0])
        .await?;
    info!(
        sender = sender.pubkey().to_string(),
        recipient = recipient.to_string(),
        mint = mint.to_string(),
        "Transfer [Allocate Proof Accounts] with signature={:?}",
        signature_1
    );

    let signature_2 = rpc_client
        .send_and_confirm_transaction(&transactions[1])
        .await?;
    info!(
        sender = sender.pubkey().to_string(),
        recipient = recipient.to_string(),
        mint = mint.to_string(),
        "Transfer [Encode Range Proof] with signature={:?}",
        signature_2
    );

    let signature_3 = rpc_client
        .send_and_confirm_transaction(&transactions[2])
        .await?;
    info!(
        sender = sender.pubkey().to_string(),
        recipient = recipient.to_string(),
        mint = mint.to_string(),
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
        sender = sender.pubkey().to_string(),
        recipient = recipient.to_string(),
        mint = mint.to_string(),
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

    // TODO: break this out to add a memo ix for funsies
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

    // sender is signer as context state authority (and fee payer, if designated)
    Ok(vec![
        close_equality_proof_instruction,
        close_ciphertext_validity_proof_instruction,
        close_range_proof_instruction,
    ])
}

/// Generates the three ZK proof-setup transactions required before a confidential transfer.
///
/// # Background: Why ZK proofs are needed
///
/// In SPL Token-2022's confidential transfer extension, token balances are encrypted
/// using **twisted ElGamal encryption** -- a variant of standard ElGamal where each
/// ciphertext is split into:
///   - A **Pedersen commitment** of the amount (independent of any public key)
///   - A **decryption handle** that binds encryption randomness to a specific
///     ElGamal public key (independent of the actual amount)
///
/// Because balances are encrypted, the on-chain program cannot directly inspect them.
/// Instead, the sender must supply **zero-knowledge proofs** that convince the program
/// the transfer is valid without revealing the actual amounts. A confidential transfer
/// requires three types of proof:
///
/// 1. **Equality proof** (ciphertext-commitment):
///    The sender knows the plaintext of its encrypted balance, but not the Pedersen
///    opening (randomness) for that ciphertext. So the sender decrypts locally, creates
///    a fresh Pedersen commitment on the new source balance (`new_source_commitment`),
///    and proves that the encrypted balance minus the transfer amount and the new
///    commitment encode the **same value**.
///
/// 2. **Ciphertext validity proof**:
///    Proves that the transfer ciphertexts (`ciphertext_lo`, `ciphertext_hi`) are
///    well-formed twisted ElGamal ciphertexts -- i.e., they were honestly encrypted
///    under the recipient's (and optionally the auditor's) ElGamal public key.
///
/// 3. **Range proof** (Bulletproofs):
///    Proves that the encrypted amounts are non-negative and within the valid range,
///    preventing underflow attacks (e.g., sending more than you have). The transfer
///    amount is split into `ciphertext_lo` (low 16 bits) and `ciphertext_hi` (high
///    bits), so the range proof covers multiple committed values.
///
/// # Why three separate transactions?
///
/// These proofs are too large to fit in a single Solana transaction. The "split proof"
/// approach stores each proof in its own on-chain **context state account**. This
/// function produces three transactions:
///
/// - **tx1**: Allocate all three context state accounts (equality, ciphertext validity,
///   range proof). Each new account keypair must sign this transaction.
/// - **tx2**: Submit and verify the range proof (largest proof, needs its own tx).
/// - **tx3**: Submit and verify the equality proof and ciphertext validity proof
///   (small enough to share a tx).
///
/// After these three transactions confirm, the caller invokes the actual
/// `confidential_transfer_transfer` instruction, which references the proof accounts
/// by pubkey. Once the transfer is done, the proof accounts are closed to reclaim rent.
///
/// # Arguments
/// * `rpc_client` - Solana RPC client for fetching on-chain state and blockhashes
/// * `sender` - Signing keypair that owns the source token account and pays fees
/// * `recipient` - Public key of the destination wallet
/// * `confidential_transfer_amount` - Raw token amount to transfer (before decimals)
/// * `mint` - SPL Token-2022 mint address (must have confidential transfer extension)
/// * `decimals` - Mint decimal precision
/// * `sender_confidential_keys` - Pre-derived ElGamal keypair (for encryption/decryption)
///   and authenticated encryption (AE) key (for local balance decryption)
///
/// # Returns
/// A tuple of `(transactions, TransferContext)` where `transactions` is `[tx1, tx2, tx3]`
/// and `TransferContext` carries all the pubkeys and proof data needed by the subsequent
/// transfer and close-account steps.
async fn execute_proof_transactions_with_keys(
    rpc_client: Arc<RpcClient>,
    sender: Arc<dyn Signer + Send + Sync>,
    recipient: &Pubkey,
    confidential_transfer_amount: u64,
    mint: &Pubkey,
    decimals: u8,
    sender_confidential_keys: &ConfidentialKeys,
) -> Result<(Vec<Transaction>, TransferContext)> {
    // ---------------------------------------------------------------------------
    // 1. Derive associated token addresses (ATAs) for sender and recipient
    // ---------------------------------------------------------------------------
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

    // The sender is the authority that can later close the proof context state accounts
    // (reclaiming their rent).
    let context_state_authority = &sender;

    // ---------------------------------------------------------------------------
    // 2. Create fresh keypairs for the three proof context state accounts
    //
    //    Each ZK proof is stored in its own on-chain account. We generate ephemeral
    //    keypairs here; their pubkeys become the account addresses. These accounts
    //    are created in tx1, populated in tx2/tx3, referenced during the transfer,
    //    and finally closed to reclaim rent.
    // ---------------------------------------------------------------------------
    let equality_proof_context_state_account = Keypair::new();
    let equality_proof_pubkey = equality_proof_context_state_account.pubkey();

    let ciphertext_validity_proof_context_state_account = Keypair::new();
    let ciphertext_validity_proof_pubkey = ciphertext_validity_proof_context_state_account.pubkey();

    let range_proof_context_state_account = Keypair::new();
    let range_proof_pubkey = range_proof_context_state_account.pubkey();

    // ---------------------------------------------------------------------------
    // 3. Fetch sender's on-chain confidential transfer state
    //
    //    The ConfidentialTransferAccount extension on the sender's token account
    //    holds the encrypted available balance and pending balance. We wrap it in
    //    TransferAccountInfo which provides helper methods for proof generation.
    // ---------------------------------------------------------------------------
    let sender_token_account_info = token
        .get_account_info(&sender_associated_token_address)
        .await?;

    let sender_account_extension_data =
        sender_token_account_info.get_extension::<ConfidentialTransferAccount>()?;

    let sender_transfer_account_info = TransferAccountInfo::new(sender_account_extension_data);

    // ---------------------------------------------------------------------------
    // 4. Fetch the recipient's ElGamal public key
    //
    //    The recipient's token account also has a ConfidentialTransferAccount extension
    //    containing their ElGamal public key. We need this key to encrypt the transfer
    //    amount so that only the recipient can decrypt it.
    // ---------------------------------------------------------------------------
    let recipient_account = token
        .get_account(recipient_associated_token_address)
        .await?;

    let recipient_elgamal_pubkey: elgamal::ElGamalPubkey =
        StateWithExtensionsOwned::<Account>::unpack(recipient_account.data)?
            .get_extension::<ConfidentialTransferAccount>()?
            .elgamal_pubkey
            .try_into()?;

    // ---------------------------------------------------------------------------
    // 5. Fetch the auditor's ElGamal public key from the mint
    //
    //    The mint's ConfidentialTransferMint extension can specify an optional auditor
    //    public key. When present, transfer ciphertexts are also encrypted under this
    //    key, allowing a designated auditor to decrypt all transfers for compliance
    //    purposes without needing the sender's or recipient's private keys.
    // ---------------------------------------------------------------------------
    let mint_account = token.get_account(*mint).await?;

    let auditor_elgamal_pubkey_option = Option::<PodElGamalPubkey>::from(
        StateWithExtensionsOwned::<Mint>::unpack(mint_account.data)?
            .get_extension::<ConfidentialTransferMint>()?
            .auditor_elgamal_pubkey,
    );

    let auditor_elgamal_pubkey: elgamal::ElGamalPubkey = auditor_elgamal_pubkey_option
        .ok_or(anyhow::anyhow!("No Auditor ElGamal pubkey"))?
        .try_into()?;

    // ---------------------------------------------------------------------------
    // 6. Generate the three split transfer proofs (client-side cryptography)
    //
    //    `generate_split_transfer_proof_data` does the heavy lifting:
    //      - Decrypts the sender's current encrypted balance using the ElGamal keypair
    //        and AE key.
    //      - Computes the new source balance (current - transfer_amount).
    //      - Splits the transfer amount into low 16 bits (ciphertext_lo) and remaining
    //        high bits (ciphertext_hi) to keep range proofs efficient.
    //      - Encrypts the split amounts under the recipient's and auditor's ElGamal keys.
    //      - Produces three proof objects:
    //          * equality_proof_data   -- proves new_source_commitment matches the
    //                                     encrypted remaining balance
    //          * ciphertext_validity   -- proves ciphertext_lo/hi are well-formed
    //                                     under the recipient/auditor keys
    //          * range_proof_data      -- proves all committed values are non-negative
    // ---------------------------------------------------------------------------
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

    // ---------------------------------------------------------------------------
    // 7. Build create + verify instruction pairs for each proof
    //
    //    Each proof needs two instructions:
    //      - A "create" instruction that allocates the context state account and
    //        writes the proof data into it.
    //      - A "verify" instruction that invokes the ZK ElGamal proof program to
    //        cryptographically verify the proof on-chain.
    //
    //    The create and verify steps are split across transactions because the
    //    account must exist before it can be verified, and transaction size limits
    //    prevent bundling everything into one tx.
    // ---------------------------------------------------------------------------
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

    // ---------------------------------------------------------------------------
    // 8. Bundle instructions into three transactions
    //
    //    tx1 - Allocate all three context state accounts. Each account keypair
    //          must sign because Solana requires the private key of a new account
    //          to authorize its creation.
    //
    //    tx2 - Verify the range proof (Bulletproofs). This is the largest proof
    //          and needs a dedicated transaction due to size constraints.
    //
    //    tx3 - Verify the equality proof and ciphertext validity proof. These are
    //          smaller sigma-protocol proofs that fit together in one transaction.
    // ---------------------------------------------------------------------------
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

    // ---------------------------------------------------------------------------
    // 9. Package proof metadata for the transfer step
    //
    //    The ciphertext validity proof account carries extra data alongside its
    //    pubkey: the split ciphertexts (lo/hi). The transfer instruction needs these
    //    to reconstruct the full encrypted transfer amount on-chain.
    //      ciphertext_lo = encryption of the low 16 bits of the transfer amount
    //      ciphertext_hi = encryption of the remaining high bits
    //    Full encrypted amount = ciphertext_lo + 2^16 * ciphertext_hi
    // ---------------------------------------------------------------------------
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
        recipient: *recipient,
        recipient_associated_token_address,
        sender_transfer_account_info,
        sender_confidential_keys: sender_confidential_keys.clone(),
        recipient_elgamal_pubkey,
        auditor_elgamal_pubkey,
    };

    Ok((vec![tx1, tx2, tx3], ctx))
}

/// Execute proof transactions - derives confidential keys from the sender signer
/// In a non-custodial flow, we cannot execute the transactions server side
async fn execute_proof_transactions(
    rpc_client: Arc<RpcClient>,
    sender: Arc<dyn Signer + Send + Sync>,
    recipient: &Pubkey,
    confidential_transfer_amount: u64,
    mint: &Pubkey,
    decimals: u8,
) -> Result<(Vec<Transaction>, TransferContext)> {
    let sender_confidential_keys = confidential_keys_for_mint(sender.clone(), mint)?;
    execute_proof_transactions_with_keys(
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
