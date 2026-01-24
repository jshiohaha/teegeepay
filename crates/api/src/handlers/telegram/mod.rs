pub mod auth;

use crate::AppState;
use axum::{Router, routing::post};
use std::sync::Arc;

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth", post(auth::handler))
        .with_state(state)
}
