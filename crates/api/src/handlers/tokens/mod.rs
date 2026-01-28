use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

use crate::AppState;

pub mod create;
pub mod mint;
pub mod supply;

/// nested within /tokens prefix
pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // .route("/{address}", get(get_token::handler))
        .route("/", post(create::handler))
        .route("/{address}/mint", post(mint::handler))
        .route("/{mint}/supply", get(supply::handler))
        .with_state(state)
}
