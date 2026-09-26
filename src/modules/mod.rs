pub mod api_keys;
pub mod audit_log;
pub mod auth;
pub mod avatars;
pub mod health;
pub mod mail;
pub mod media;
pub mod oauth;
pub mod passkeys;
pub mod qr_login;
pub mod users;

use std::time::Duration;

use axum::Router;

use crate::{common::middleware::rate_limit::RateLimiter, state::AppState};

/// Feature routes, to be nested under the API prefix.
pub fn router(state: &AppState) -> Router<AppState> {
    let strict = state.rate_limiter.clone();
    // Polling a QR login is frequent by design, so it gets its own, higher limit.
    let qr_poll = RateLimiter::new(
        "qr_poll",
        state.config.qr_login.poll_rate_limit_per_minute,
        Duration::from_secs(60),
        state.config.account.trust_proxy_headers,
    );

    Router::new()
        .nest("/auth", auth::router(strict.clone()))
        .nest("/auth/oauth", oauth::router(strict.clone()))
        .nest("/auth/passkeys", passkeys::router(strict.clone()))
        .nest("/auth/qr", qr_login::router(strict, qr_poll))
        .nest("/users", users::router())
        .nest("/users/me/avatar", avatars::own_router())
        .nest("/avatars", avatars::public_router())
        .nest("/media", media::router())
        .nest("/api-keys", api_keys::router())
        .nest("/audit-log", audit_log::router())
}
