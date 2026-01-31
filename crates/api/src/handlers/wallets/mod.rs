use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

use crate::AppState;

pub mod balance;
pub mod create;
pub mod deposit;
pub mod list;
pub mod withdraw;

/// nested within /wallets prefix
pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list::handler))
        .route("/{address}/balance", get(balance::handler))
        .route("/{address}/balance/solana", get(balance::solana))
        .route("/", post(create::handler))
        .route("/{address}/deposit", post(deposit::handler))
        .route("/{address}/withdraw", post(withdraw::handler))
        .with_state(state)
}
