mod db;
mod handlers;
mod models;
mod partial_sign;
mod routes;
mod solana;

use crate::solana::airdrop::request_and_confirm;
use anyhow::Result;
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::CommitmentConfig};
use solana_keypair::Keypair;
use solana_signer::Signer;
use spl_token_2022::solana_zk_sdk::encryption::elgamal::ElGamalKeypair;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct AppState {
    pub db: sqlx::PgPool,
    pub rpc_client: Arc<RpcClient>,
    pub elgamal_keypair: Arc<ElGamalKeypair>,
    pub global_authority: Arc<Keypair>,
}

// TODO: EOD
// transfer works :D
//
// - create link to initiate transfer
// --> create sender confidential ata
// --> deposit n tokens
// --> apply pending balance
// --> create proofs
// --> execute transfer

// DONE
// - setup confidential ata for user and mint (used for setting up for recipients)
// - make endpoint to create a mint
// - make endpoint to mint tokens to a wallet
// - check balance
// - withdraw tokens?
// --> baked in apply pending balance for now

// TODO: LATER
// - make endpoint to create a confidential mint
// - make endpoint to mint confidential tokens to a wallet

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set");
    info!("RPC client created for URL: {:?}", &rpc_url);
    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        rpc_url,
        CommitmentConfig::confirmed(),
    ));

    let elgamal_keypair = solana::utils::create_keypair_elgamal();
    let global_authority = solana::utils::create_keypair();
    request_and_confirm(
        rpc_client.clone(),
        &global_authority.pubkey(),
        10 * 10_u64.pow(9),
    )
    .await?;

    let state = Arc::new(AppState {
        db: pool,
        rpc_client: rpc_client.clone(),
        elgamal_keypair: Arc::new(elgamal_keypair),
        global_authority: Arc::new(global_authority),
    });

    let app = routes::create_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("Server listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}
