use axum::{Router, routing::post};
use std::sync::Arc;

use crate::AppState;

pub mod create;

/// nested within /transfers prefix
pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(create::handler))
        .with_state(state)
}
