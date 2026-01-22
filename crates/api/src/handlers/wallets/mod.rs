use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

use crate::AppState;

pub mod balance;
pub mod create;
pub mod list;
pub mod withdraw;

/// nested within /wallets prefix
pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list::handler))
        .route("/{address}/balance", get(balance::handler))
        .route("/", post(create::handler))
        .route("/{address}/withdraw", post(withdraw::handler))
        .with_state(state)
}
