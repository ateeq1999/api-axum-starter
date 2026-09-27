use axum::{
    Router, middleware,
    routing::{delete, get, post},
};

use crate::{
    common::middleware::rate_limit::{self, RateLimiter},
    state::AppState,
};

pub mod delete_passkey;
pub mod list_passkeys;
pub mod register_passkey;
pub mod sign_in_with_passkey;

use delete_passkey::remove;
use list_passkeys::list;
use register_passkey::{begin_registration, finish_registration};
use sign_in_with_passkey::{begin_login, finish_login};

/// Mounted at `/api/v1/auth/passkeys`.
///
/// Managing passkeys needs an interactive sign-in (not an API key). Signing in with one is public
/// and rate limited.
pub fn router(limiter: RateLimiter) -> Router<AppState> {
    let public = Router::new()
        .route("/login/begin", post(begin_login))
        .route("/login/finish", post(finish_login))
        .route_layer(middleware::from_fn_with_state(limiter, rate_limit::enforce));

    public
        .route("/", get(list))
        .route("/register/begin", post(begin_registration))
        .route("/register/finish", post(finish_registration))
        .route("/{id}", delete(remove))
}

// WebAuthn ceremony payloads (`webauthn-rs` types) are opaque, browser-generated JSON blobs not
// meant for manual construction, so they are documented as plain objects here rather than fully
// modeled field-by-field.
