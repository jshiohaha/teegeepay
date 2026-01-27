use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use spl_associated_token_account::{
    get_associated_token_address_with_program_id,
    instruction::create_associated_token_account_idempotent,
};
use spl_token_2022::{
    error::TokenError,
    extension::{
        BaseStateWithExtensions, ExtensionType, StateWithExtensionsOwned,
        confidential_mint_burn::ConfidentialMintBurn,
        confidential_transfer::{
            ConfidentialTransferAccount, ConfidentialTransferMint,
            instruction::{PubkeyValidityProofData, configure_account},
        },
    },
    instruction::reallocate,
    state::{Account, Mint},
};
use spl_token_confidential_transfer_proof_extraction::instruction::ProofLocation;
use std::sync::Arc;

use crate::solana::GeneratedInstructions;
use crate::solana::signature_signer::ConfidentialKeys;

pub async fn is_confidential_mint_enabled(
    rpc_client: Arc<RpcClient>,
    mint: &Pubkey,
) -> Result<bool> {
    let mint_account = rpc_client.get_account(mint).await?;
    let mint_state = StateWithExtensionsOwned::<Mint>::unpack(mint_account.data)?;
    Ok(mint_state
        .get_extension::<ConfidentialTransferMint>()
        .is_ok())
}

pub async fn is_confidential_mintburn_enabled(
    rpc_client: Arc<RpcClient>,
    mint: &Pubkey,
) -> Result<bool> {
    let mint_account = rpc_client.get_account(mint).await?;
    let mint_state = StateWithExtensionsOwned::<Mint>::unpack(mint_account.data)?;
    Ok(mint_state.get_extension::<ConfidentialMintBurn>().is_ok())
}

#[allow(dead_code)]
pub async fn is_token_account_initialized(
    rpc_client: Arc<RpcClient>,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Result<bool> {
    let ata = get_associated_token_address_with_program_id(owner, mint, &spl_token_2022::id());
    let ata_account = rpc_client.get_account(&ata).await?;
    let token_account = StateWithExtensionsOwned::<Account>::unpack(ata_account.data)?;
    Ok(token_account.base.owner == *owner && token_account.base.mint == *mint)
}

pub async fn get_maybe_ata(
    rpc_client: Arc<RpcClient>,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Result<(Pubkey, Option<solana_account::Account>)> {
    let ata = get_associated_token_address_with_program_id(
        &owner, // Token account owner
        &mint,  // Mint
        &spl_token_2022::id(),
    );

    let maybe_ata_account = match rpc_client.get_account(&ata).await {
        Ok(account) => Some(account),
        Err(err) => {
            let is_missing = err.to_string().contains("AccountNotFound")
                || err.to_string().contains("could not find account");
            if is_missing {
                None
            } else {
                return Err(err.into());
            }
        }
    };

    Ok((ata, maybe_ata_account))
}

pub fn ata_has_confidential_transfer_extension(
    maybe_ata_account: Option<solana_account::Account>,
    ata_authority: &Pubkey,
    mint: &Pubkey,
) -> Result<bool> {
    let mut requires_confidential_extension = true;
    if let Some(ata_account) = &maybe_ata_account {
        if ata_account.owner != spl_token_2022::id() {
            return Err(anyhow::anyhow!(
                "Associated token account is not owned by Token-2022 program"
            ));
        }

        let token_account = StateWithExtensionsOwned::<Account>::unpack(ata_account.data.clone())?;
        if token_account.base.owner != *ata_authority {
            return Err(anyhow::anyhow!(
                "Associated token account owner does not match authority"
            ));
        }

        if token_account.base.mint != *mint {
            return Err(anyhow::anyhow!(
                "Associated token account mint does not match"
            ));
        }

        requires_confidential_extension = token_account
            .get_extension::<ConfidentialTransferAccount>()
            .is_err();
    }

    Ok(requires_confidential_extension)
}

/// Sets up a token account with confidential transfer extension.
///
/// # Arguments
/// * `rpc_client` - RPC client for Solana
/// * `fee_payer` - Public key of the fee payer
/// * `ata_authority_pubkey` - Public key of the token account owner
/// * `mint` - Mint address
/// * `confidential_keys` - Pre-derived ElGamal and AE keypairs for confidential transfers
///
/// # Browser Wallet Flow
/// The `confidential_keys` can be derived from a browser wallet signature:
/// ```ignore
/// // Client side: wallet.signMessage(ata_address_bytes)
/// // Server side:
/// let keys = ConfidentialKeys::from_signature(wallet_pubkey, signature, &ata.to_bytes())?;
/// setup_token_account_with_keys(rpc_client, fee_payer, wallet_pubkey, mint, keys).await?;
/// ```
pub async fn setup_token_account_with_keys(
    rpc_client: Arc<RpcClient>,
    fee_payer: &Pubkey,
    ata_authority_pubkey: &Pubkey,
    mint: &Pubkey,
    confidential_keys: &ConfidentialKeys,
) -> Result<GeneratedInstructions> {
    let (ata, maybe_ata_account) =
        get_maybe_ata(rpc_client.clone(), ata_authority_pubkey, mint).await?;

    let mut instructions = Vec::new();
    if maybe_ata_account.is_none() {
        instructions.push(create_associated_token_account_idempotent(
            fee_payer,            // Funding account
            ata_authority_pubkey, // Token account owner
            mint,                 // Mint
            &spl_token_2022::id(),
        ));
    }

    let requires_confidential_extension =
        ata_has_confidential_transfer_extension(maybe_ata_account, ata_authority_pubkey, mint)
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to check if ATA has confidential transfer extension: {}",
                    e
                )
            })?;

    if requires_confidential_extension {
        // Instruction to reallocate the token account to include the `ConfidentialTransferAccount` extension
        let reallocate_instruction = reallocate(
            &spl_token_2022::id(),
            &ata,                                          // Token account
            fee_payer,                                     // Payer
            ata_authority_pubkey,                          // Token account owner
            &[ata_authority_pubkey],                       // Signers
            &[ExtensionType::ConfidentialTransferAccount], // Extension to reallocate space for
        )?;

        // The maximum number of `Deposit` and `Transfer` instructions that can
        // credit `pending_balance` before the `ApplyPendingBalance` instruction is executed
        let maximum_pending_balance_credit_counter = 65536;

        // Initial token balance is 0
        let decryptable_balance = confidential_keys.ae_key.encrypt(0);

        // The instruction data that is needed for the `ProofInstruction::VerifyPubkeyValidity` instruction.
        // It includes the cryptographic proof as well as the context data information needed to verify the proof.
        // Generating the proof data client-side (instead of using a separate proof account)
        let proof_data = PubkeyValidityProofData::new(&confidential_keys.elgamal_keypair)
            .map_err(|_| TokenError::ProofGeneration)?;

        // `InstructionOffset` indicates that proof is included in the same transaction
        // This means that the proof instruction offset must be always be 1.
        let proof_location = ProofLocation::InstructionOffset(1.try_into().unwrap(), &proof_data);

        // Instructions to configure the token account, including the proof instruction
        // Appends the `VerifyPubkeyValidityProof` instruction right after the `ConfigureAccount` instruction.
        let configure_account_instruction = configure_account(
            &spl_token_2022::id(),                  // Program ID
            &ata,                                   // Token account
            mint,                                   // Mint
            &decryptable_balance.into(),            // Initial balance
            maximum_pending_balance_credit_counter, // Maximum pending balance credit counter
            ata_authority_pubkey,                   // Token Account Owner
            &[],                                    // Additional signers
            proof_location,                         // Proof location
        )?;

        instructions.push(reallocate_instruction);
        instructions.extend(configure_account_instruction);
    }

    Ok(GeneratedInstructions {
        instructions,
        additional_signers: vec![],
    })
}

#[cfg(test)]
mod tests {

    use super::*;
    use solana_keypair::Keypair;
    use solana_signer::Signer;
    use std::sync::Arc;

    use crate::solana::utils::confidential_keys_for_mint;

    #[tokio::test]
    #[ignore]
    async fn test_setup_token_account() -> Result<()> {
        let sender_keypair = Arc::new(Keypair::new());
        let fee_payer = Keypair::new();
        let mint = Keypair::new();
        let rpc_client = Arc::new(RpcClient::new("http://localhost:8899".to_string()));

        let confidential_keys = confidential_keys_for_mint(sender_keypair.clone(), &mint.pubkey())?;

        setup_token_account_with_keys(
            rpc_client,
            &fee_payer.pubkey(),
            &sender_keypair.pubkey(),
            &mint.pubkey(),
            &confidential_keys,
        )
        .await?;
        Ok(())
    }
}
