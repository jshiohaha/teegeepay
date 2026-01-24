mod auth;
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
use spl_token_2022::solana_zk_sdk::encryption::elgamal::{ElGamalKeypair, ElGamalSecretKey};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct AppState {
    pub db: sqlx::PgPool,
    pub rpc_client: Arc<RpcClient>,
    pub elgamal_keypair: Arc<ElGamalKeypair>,
    pub global_authority: Arc<Keypair>,
    pub telegram_bot_token: String,
    pub jwt_secret: String,
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

    let auditor_kp = std::env::var("AUDITOR_KP").expect("AUDITOR_KP must be set");
    let auditor_kp = solana::utils::kp_from_base58_string(&auditor_kp);
    let elgamal_keypair = solana::utils::el_gamal_deterministic(&auditor_kp)
        .map_err(|e| anyhow::anyhow!("Failed to create ElGamal keypair: {}", e))?;

    let authority_kp = std::env::var("AUTHORITY_KP").expect("AUTHORITY_KP must be set");
    let global_authority = solana::utils::kp_from_base58_string(&authority_kp);

    request_and_confirm(
        rpc_client.clone(),
        &global_authority.pubkey(),
        10 * 10_u64.pow(9),
    )
    .await?;

    let telegram_bot_token =
        std::env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN must be set");
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let state = Arc::new(AppState {
        db: pool,
        rpc_client: rpc_client.clone(),
        elgamal_keypair: Arc::new(elgamal_keypair),
        global_authority: Arc::new(global_authority),
        telegram_bot_token,
        jwt_secret,
    });

    let app = routes::create_router(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3000);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("Server listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}
