use {
    crate::solana::GeneratedInstructions,
    anyhow::Result,
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

pub async fn create_mint(
    rpc_client: Arc<RpcClient>,
    fee_payer: Arc<dyn Signer + Send + Sync>,
    // mint and freeze authority are the same for now
    absolute_authority: Arc<Keypair>,
    auditor_elgamal_keypair: Arc<ElGamalKeypair>,
    supply_aes_key: Arc<AeKey>,
    mint: Option<Arc<Keypair>>,
    decimals: Option<u8>,
    name: String,
    symbol: String,
    metadata_uri: Option<String>,
) -> Result<GeneratedInstructions> {
    let mint = mint.unwrap_or_else(|| Arc::new(Keypair::new()));

    let mint_authority = absolute_authority.clone();
    let freeze_authority = absolute_authority.clone();
    let decimals = decimals.unwrap_or(9);
    let metadata_uri = metadata_uri.unwrap_or_default();

    let metadata_state = TokenMetadata {
        mint: mint.pubkey(),
        name: name.clone(),
        symbol: symbol.clone(),
        uri: metadata_uri.clone(),
        ..Default::default()
    };
    let metadata_extension_space = metadata_state
        .tlv_size_of()
        .map_err(|e| anyhow::anyhow!("Failed to size token metadata: {}", e))?;

    // Confidential Transfer Extension authority
    // Authority to modify the `ConfidentialTransferMint` configuration and to approve new accounts (if `auto_approve_new_accounts` is false?)
    let confidential_transfer_authority = absolute_authority.clone();

    // ConfidentialTransferMint extension parameters
    let confidential_transfer_mint_extension =
        ExtensionInitializationParams::ConfidentialTransferMint {
            authority: Some(confidential_transfer_authority.pubkey()),
            auto_approve_new_accounts: true, // If `true`, no approval is required and new accounts may be used immediately
            auditor_elgamal_pubkey: Some((*auditor_elgamal_keypair.pubkey()).into()),
        };

    // Calculate the space required for the mint account with the extensions
    let base_space = ExtensionType::try_calculate_account_len::<Mint>(&[
        ExtensionType::ConfidentialTransferMint,
        ExtensionType::ConfidentialMintBurn,
        ExtensionType::MetadataPointer,
    ])?;
    let final_space = base_space
        .checked_add(metadata_extension_space)
        .ok_or_else(|| anyhow::anyhow!("Mint space calculation overflowed"))?;
    let space = base_space;

    // Calculate the lamports required for the mint account
    let rent = rpc_client
        .get_minimum_balance_for_rent_exemption(space)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get minimum balance for rent exemption: {}", e))?;
    let final_rent = rpc_client
        .get_minimum_balance_for_rent_exemption(final_space)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to get minimum balance for final rent exemption: {}",
                e
            )
        })?;
    let additional_rent = final_rent.saturating_sub(rent);

    // Instructions to create the mint account
    let create_account_instruction = create_account(
        &fee_payer.pubkey(),
        &mint.pubkey(),
        rent,
        space as u64,
        &spl_token_2022::id(),
    );

    // ConfidentialTransferMint extension instruction
    let confidential_transfer_instruction =
        confidential_transfer_mint_extension.instruction(&spl_token_2022::id(), &mint.pubkey())?;

    let pod_auditor_elgamal_keypair: PodElGamalPubkey =
        auditor_elgamal_keypair.pubkey_owned().into();
    let decryptable_supply_ciphertext = supply_aes_key.encrypt(0).into();
    let extension_mintburn_init_instruction = confidential_mint_burn::instruction::initialize_mint(
        &spl_token_2022::id(),
        &mint.pubkey(),
        &pod_auditor_elgamal_keypair,
        &decryptable_supply_ciphertext,
    )?;

    let metadata_pointer_extension = ExtensionInitializationParams::MetadataPointer {
        authority: Some(mint_authority.pubkey()),
        metadata_address: Some(mint.pubkey()),
    };
    let metadata_pointer_instruction =
        metadata_pointer_extension.instruction(&spl_token_2022::id(), &mint.pubkey())?;

    let mut instructions = vec![
        create_account_instruction,
        confidential_transfer_instruction,
        extension_mintburn_init_instruction,
        metadata_pointer_instruction,
    ];

    // Initialize the mint account
    //TODO: Use program-2022/src/extension/confidential_transfer/instruction/initialize_mint()
    let initialize_mint_instruction = initialize_mint(
        &spl_token_2022::id(),
        &mint.pubkey(),
        &mint_authority.pubkey(),
        Some(&freeze_authority.pubkey()),
        decimals,
    )?;

    instructions.push(initialize_mint_instruction);

    if additional_rent > 0 {
        let transfer_rent_instruction =
            transfer(&fee_payer.pubkey(), &mint.pubkey(), additional_rent);
        instructions.push(transfer_rent_instruction);
    }

    let initialize_metadata_instruction = spl_token_metadata_interface::instruction::initialize(
        &spl_token_2022::id(),
        &mint.pubkey(),
        &mint_authority.pubkey(),
        &mint.pubkey(),
        &mint_authority.pubkey(),
        name,
        symbol,
        metadata_uri,
    );

    instructions.push(initialize_metadata_instruction);

    let mut additional_signers: Vec<Arc<dyn Signer + Send + Sync>> = vec![mint.clone()];
    if fee_payer.pubkey() != mint_authority.pubkey() {
        additional_signers.push(mint_authority);
    }

    Ok(GeneratedInstructions {
        instructions,
        additional_signers,
    })
}
