use axum::{Router, routing::get};

use crate::health;

pub fn build_router() -> Router {
    Router::new().route("/health/live", get(health::liveness))
}
