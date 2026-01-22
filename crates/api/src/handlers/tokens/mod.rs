use axum::{Router, routing::post};
use std::sync::Arc;

use crate::AppState;

pub mod create;
pub mod mint;

/// nested within /tokens prefix
pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // .route("/{address}", get(get_token::handler))
        .route("/", post(create::handler))
        .route("/{address}/mint", post(mint::handler))
        .with_state(state)
}
