//! SPL Token-2022 mint creation with confidential transfer extensions.
//!
//! Builds the full instruction sequence for creating a new mint account
//! with the ConfidentialTransferMint extension, optional ConfidentialMintBurn
//! extension, on-chain token metadata, and rent funding — all in a single
//! composable set of instructions returned via [`GeneratedInstructions`].

use {
    crate::solana::GeneratedInstructions,
    anyhow::{Context, Result},
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_system_interface::instruction::{create_account, transfer},
    spl_token_2022::{
        extension::{ExtensionType, confidential_mint_burn},
        instruction::initialize_mint,
        solana_zk_sdk::encryption::{
            auth_encryption::AeKey, elgamal::ElGamalKeypair, pod::elgamal::PodElGamalPubkey,
        },
        state::Mint,
    },
    spl_token_client::token::ExtensionInitializationParams,
    spl_token_metadata_interface::state::TokenMetadata,
    std::sync::Arc,
};

/// Parameters for enabling the confidential mint-burn extension.
pub struct ConfidentialMintBurnParams {
    pub supply_aes_key: Arc<AeKey>,
}

/// Parameters for creating a new SPL Token-2022 mint.
pub struct CreateMintParams {
    pub rpc_client: Arc<RpcClient>,
    pub fee_payer: Arc<dyn Signer + Send + Sync>,
    /// Used as mint authority, freeze authority, and confidential transfer authority.
    pub authority: Arc<Keypair>,
    /// ElGamal keypair for auditing confidential operations.
    pub auditor_elgamal_keypair: Arc<ElGamalKeypair>,
    /// Keypair for the mint account. A new one is generated if `None`.
    pub mint: Option<Arc<Keypair>>,
    /// Token decimals. Defaults to 9.
    pub decimals: Option<u8>,
    pub name: String,
    pub symbol: String,
    pub metadata_uri: Option<String>,
    /// Enables the ConfidentialMintBurn extension when `Some`.
    pub confidential_mint_burn: Option<ConfidentialMintBurnParams>,
}

pub async fn create_mint(params: CreateMintParams) -> Result<GeneratedInstructions> {
    let CreateMintParams {
        rpc_client,
        fee_payer,
        authority,
        auditor_elgamal_keypair,
        mint,
        decimals,
        name,
        symbol,
        metadata_uri,
        confidential_mint_burn,
    } = params;

    let mint = mint.unwrap_or_else(|| Arc::new(Keypair::new()));
    let decimals = decimals.unwrap_or(9);
    let metadata_uri = metadata_uri.unwrap_or_default();

    let mut extension_types = vec![ExtensionType::ConfidentialTransferMint];
    if confidential_mint_burn.is_some() {
        extension_types.push(ExtensionType::ConfidentialMintBurn);
    }
    extension_types.push(ExtensionType::MetadataPointer);

    let metadata_state = TokenMetadata {
        mint: mint.pubkey(),
        name: name.clone(),
        symbol: symbol.clone(),
        uri: metadata_uri.clone(),
        ..Default::default()
    };
    let metadata_extension_space = metadata_state
        .tlv_size_of()
        .map_err(|e| anyhow::anyhow!("Failed to size token metadata: {e}"))?;

    let base_space = ExtensionType::try_calculate_account_len::<Mint>(&extension_types)?;
    let total_space = base_space
        .checked_add(metadata_extension_space)
        .context("Mint space calculation overflowed")?;

    let base_rent = rpc_client
        .get_minimum_balance_for_rent_exemption(base_space)
        .await
        .context("Failed to get rent exemption for base space")?;
    let total_rent = rpc_client
        .get_minimum_balance_for_rent_exemption(total_space)
        .await
        .context("Failed to get rent exemption for total space")?;
    let additional_rent = total_rent.saturating_sub(base_rent);

    let token_program = &spl_token_2022::id();
    let mint_pubkey = &mint.pubkey();

    let mut instructions = vec![
        // 1. Create the mint account
        create_account(
            &fee_payer.pubkey(),
            mint_pubkey,
            base_rent,
            base_space as u64,
            token_program,
        ),
        // 2. ConfidentialTransferMint extension
        ExtensionInitializationParams::ConfidentialTransferMint {
            authority: Some(authority.pubkey()),
            auto_approve_new_accounts: true,
            auditor_elgamal_pubkey: Some((*auditor_elgamal_keypair.pubkey()).into()),
        }
        .instruction(token_program, mint_pubkey)?,
    ];

    // 3. ConfidentialMintBurn extension (conditional)
    if let Some(ref cmb) = confidential_mint_burn {
        let pod_auditor_pubkey: PodElGamalPubkey = auditor_elgamal_keypair.pubkey_owned().into();
        let decryptable_supply = cmb.supply_aes_key.encrypt(0).into();
        instructions.push(confidential_mint_burn::instruction::initialize_mint(
            token_program,
            mint_pubkey,
            &pod_auditor_pubkey,
            &decryptable_supply,
        )?);
    }

    instructions.extend([
        // 4. MetadataPointer extension
        ExtensionInitializationParams::MetadataPointer {
            authority: Some(authority.pubkey()),
            metadata_address: Some(mint.pubkey()),
        }
        .instruction(token_program, mint_pubkey)?,
        // 5. Initialize the mint
        initialize_mint(
            token_program,
            mint_pubkey,
            &authority.pubkey(),
            Some(&authority.pubkey()),
            decimals,
        )?,
    ]);

    // 6. Transfer additional rent for metadata if needed
    if additional_rent > 0 {
        instructions.push(transfer(&fee_payer.pubkey(), mint_pubkey, additional_rent));
    }

    // 7. Initialize token metadata
    instructions.push(spl_token_metadata_interface::instruction::initialize(
        token_program,
        mint_pubkey,
        &authority.pubkey(),
        &mint.pubkey(),
        &authority.pubkey(),
        name,
        symbol,
        metadata_uri,
    ));

    let mut additional_signers: Vec<Arc<dyn Signer + Send + Sync>> = vec![mint.clone()];
    if fee_payer.pubkey() != authority.pubkey() {
        additional_signers.push(authority);
    }

    Ok(GeneratedInstructions {
        instructions,
        additional_signers,
    })
}
