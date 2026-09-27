use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::{
    common::middleware::rate_limit::{self, RateLimiter},
    state::AppState,
};

pub mod approve_session;
pub mod create_session;
pub mod poll_session;
pub mod reject_session;
pub mod scan_session;

use approve_session::approve;
use create_session::create;
use poll_session::poll;
use reject_session::reject;
use scan_session::scan;

const SECRET_HEADER: &str = "x-qr-secret";

/// Mounted at `/api/v1/auth/qr`.
///
/// * new device: `POST /sessions` (public), then poll `GET /sessions/{id}` with `X-QR-Secret`
/// * signed-in device: `POST /sessions/{id}/scan`, then `/approve` or `/reject` (interactive
///   sign-in required: an API key cannot approve a login)
pub fn router(create_limiter: RateLimiter, poll_limiter: RateLimiter) -> Router<AppState> {
    let create =
        Router::new()
            .route("/sessions", post(create))
            .route_layer(middleware::from_fn_with_state(
                create_limiter,
                rate_limit::enforce,
            ));
    let poll = Router::new()
        .route("/sessions/{id}", get(poll))
        .route_layer(middleware::from_fn_with_state(
            poll_limiter,
            rate_limit::enforce,
        ));

    create
        .merge(poll)
        .route("/sessions/{id}/scan", post(scan))
        .route("/sessions/{id}/approve", post(approve))
        .route("/sessions/{id}/reject", post(reject))
}
