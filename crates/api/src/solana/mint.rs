use crate::solana::{
    GeneratedInstructions, signature_signer::ConfidentialKeys,
    zk::get_zk_proof_context_state_account_creation_instructions,
};
use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token_2022::{
    extension::{
        BaseStateWithExtensions, StateWithExtensionsOwned,
        confidential_mint_burn::{self, ConfidentialMintBurn},
        confidential_transfer::DecryptableBalance,
    },
    instruction::mint_to,
    solana_zk_sdk::encryption::{
        auth_encryption::AeKey,
        elgamal::{ElGamalKeypair, ElGamalPubkey},
    },
    solana_zk_sdk::zk_elgamal_proof_program::instruction::{ContextStateInfo, close_context_state},
    state::Mint,
};
use spl_token_confidential_transfer_proof_extraction::instruction::ProofLocation;
use spl_token_confidential_transfer_proof_generation::mint::MintProofData;
use std::sync::Arc;

use spl_token_2022::extension::confidential_mint_burn::account_info::SupplyAccountInfo;

pub struct ConfidentialMintParams<'a> {
    pub destination_keys: &'a ConfidentialKeys,
    pub supply_elgamal_keypair: Arc<ElGamalKeypair>,
    pub supply_aes_key: Arc<AeKey>,
    pub auditor_elgamal_pubkey: Option<ElGamalPubkey>,
}

pub struct PendingTransaction {
    pub instructions: Vec<Instruction>,
    pub additional_signers: Vec<Arc<dyn Signer + Send + Sync>>,
}

pub async fn go(
    _rpc_client: Arc<RpcClient>,
    _funding_address: &Pubkey,
    mint_authority: Arc<dyn Signer + Send + Sync>,
    token_account_owner: &Pubkey,
    mint: &Pubkey,
    mint_amount: u64,
    confidential_params: Option<ConfidentialMintParams<'_>>,
) -> Result<GeneratedInstructions> {
    if confidential_params.is_some() {
        anyhow::bail!("Confidential mint requires pre-built context transactions");
    }

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

pub async fn build_confidential_mint_transactions(
    rpc_client: Arc<RpcClient>,
    payer: Arc<dyn Signer + Send + Sync>,
    destination_account: &Pubkey,
    mint: &Pubkey,
    mint_amount: u64,
    params: ConfidentialMintParams<'_>,
) -> Result<Vec<PendingTransaction>> {
    let mint_account = rpc_client
        .get_account(mint)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch mint account: {}", e))?;
    let mint_state = StateWithExtensionsOwned::<Mint>::unpack(mint_account.data)
        .map_err(|e| anyhow::anyhow!("Failed to unpack mint: {}", e))?;
    let conf_extension = mint_state
        .get_extension::<ConfidentialMintBurn>()
        .map_err(|_| anyhow::anyhow!("Mint does not support confidential mint/burn"))?;

    let supply_info = SupplyAccountInfo::new(conf_extension);
    let supply_elgamal_keypair = params.supply_elgamal_keypair.as_ref();
    let supply_aes_key = params.supply_aes_key.as_ref();
    let destination_elgamal_pubkey = params.destination_keys.elgamal_keypair.pubkey();
    let auditor_pubkey = params.auditor_elgamal_pubkey.as_ref();

    let MintProofData {
        equality_proof_data,
        ciphertext_validity_proof_data_with_ciphertext,
        range_proof_data,
    } = supply_info
        .generate_split_mint_proof_data(
            mint_amount,
            supply_elgamal_keypair,
            supply_aes_key,
            destination_elgamal_pubkey,
            auditor_pubkey,
        )
        .map_err(|e| anyhow::anyhow!("Failed to generate mint proof data: {:?}", e))?;

    let new_decryptable_supply: DecryptableBalance = supply_info
        .new_decryptable_supply(mint_amount, supply_elgamal_keypair, supply_aes_key)
        .map_err(|e| anyhow::anyhow!("Failed to compute decryptable supply: {:?}", e))?
        .into();

    let equality_ctx = Arc::new(Keypair::new());
    let range_ctx = Arc::new(Keypair::new());
    let ciphertext_ctx = Arc::new(Keypair::new());

    let payer_pubkey = payer.pubkey();

    let (range_create_ix, range_verify_ix) =
        get_zk_proof_context_state_account_creation_instructions(
            rpc_client.clone(),
            &payer_pubkey,
            &range_ctx.pubkey(),
            &payer_pubkey,
            &range_proof_data,
        )
        .await?;

    let (equality_create_ix, equality_verify_ix) =
        get_zk_proof_context_state_account_creation_instructions(
            rpc_client.clone(),
            &payer_pubkey,
            &equality_ctx.pubkey(),
            &payer_pubkey,
            &equality_proof_data,
        )
        .await?;

    let (ciphertext_create_ix, ciphertext_verify_ix) =
        get_zk_proof_context_state_account_creation_instructions(
            rpc_client,
            &payer_pubkey,
            &ciphertext_ctx.pubkey(),
            &payer_pubkey,
            &ciphertext_validity_proof_data_with_ciphertext.proof_data,
        )
        .await?;

    let mut pending_txs = Vec::new();
    pending_txs.push(PendingTransaction {
        instructions: vec![range_create_ix, equality_create_ix, ciphertext_create_ix],
        additional_signers: vec![
            range_ctx.clone() as Arc<dyn Signer + Send + Sync>,
            equality_ctx.clone() as Arc<dyn Signer + Send + Sync>,
            ciphertext_ctx.clone() as Arc<dyn Signer + Send + Sync>,
        ],
    });

    pending_txs.push(PendingTransaction {
        instructions: vec![range_verify_ix],
        additional_signers: vec![],
    });

    pending_txs.push(PendingTransaction {
        instructions: vec![equality_verify_ix, ciphertext_verify_ix],
        additional_signers: vec![],
    });

    let equality_proof_location = ProofLocation::ContextStateAccount(&equality_ctx.pubkey());
    let ciphertext_validity_proof_location =
        ProofLocation::ContextStateAccount(&ciphertext_ctx.pubkey());
    let range_proof_location = ProofLocation::ContextStateAccount(&range_ctx.pubkey());

    let mint_amount_auditor_ciphertext_lo =
        ciphertext_validity_proof_data_with_ciphertext.ciphertext_lo;
    let mint_amount_auditor_ciphertext_hi =
        ciphertext_validity_proof_data_with_ciphertext.ciphertext_hi;

    let mut mint_instructions =
        confidential_mint_burn::instruction::confidential_mint_with_split_proofs(
            &spl_token_2022::id(),
            destination_account,
            mint,
            &mint_amount_auditor_ciphertext_lo,
            &mint_amount_auditor_ciphertext_hi,
            &payer_pubkey,
            &[],
            equality_proof_location,
            ciphertext_validity_proof_location,
            range_proof_location,
            &new_decryptable_supply,
        )
        .map_err(|e| anyhow::anyhow!("Failed to build confidential mint instructions: {}", e))?;

    let destination_for_refund = payer_pubkey;
    let close_equality_ix = close_context_state(
        ContextStateInfo {
            context_state_account: &equality_ctx.pubkey(),
            context_state_authority: &payer_pubkey,
        },
        &destination_for_refund,
    );
    let close_ciphertext_ix = close_context_state(
        ContextStateInfo {
            context_state_account: &ciphertext_ctx.pubkey(),
            context_state_authority: &payer_pubkey,
        },
        &destination_for_refund,
    );
    let close_range_ix = close_context_state(
        ContextStateInfo {
            context_state_account: &range_ctx.pubkey(),
            context_state_authority: &payer_pubkey,
        },
        &destination_for_refund,
    );

    mint_instructions.push(close_equality_ix);
    mint_instructions.push(close_ciphertext_ix);
    mint_instructions.push(close_range_ix);

    pending_txs.push(PendingTransaction {
        instructions: mint_instructions,
        additional_signers: vec![],
    });

    Ok(pending_txs)
}
