use crate::AppState;
use crate::handlers;
use axum::{Router, routing::get};
use handlers::tokens::routes as token_routes;
use handlers::transfers::routes as transfer_routes;
use handlers::wallets::routes as wallet_routes;
use std::sync::Arc;

// NOTE: none of these accounts have proper authentication or anything. For demo purposes only.
pub fn create_router(state: Arc<AppState>) -> Router<()> {
    Router::new()
        .route("/health", get(handlers::health::handler))
        .nest("/wallets", wallet_routes(state.clone()))
        .nest("/transfers", transfer_routes(state.clone()))
        .nest("/tokens", token_routes(state.clone()))
        .with_state(state.clone())
}
