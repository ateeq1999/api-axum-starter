use axum::{
    Router, middleware,
    routing::{delete, get, post},
};

use super::{error::OAuthError, provider::Provider};
use crate::{
    common::middleware::rate_limit::{self, RateLimiter},
    state::AppState,
};

pub mod exchange_code;
pub mod handle_callback;
pub mod list_identities;
pub mod list_providers;
pub mod start_linking;
pub mod start_sign_in;
pub mod unlink_provider;

use exchange_code::exchange;
use handle_callback::callback;
use list_identities::identities;
use list_providers::providers;
use start_linking::link;
use start_sign_in::login;
use unlink_provider::unlink;

/// Mounted at `/api/v1/auth/oauth`.
///
/// Sign-in flow: the SPA sends the browser to `GET /{provider}/login`; the provider redirects
/// to `GET /{provider}/callback`, which redirects on to `<frontend>/oauth/callback?code=...`;
/// the SPA then posts that code to `POST /exchange` to get an access token.
pub fn router(limiter: RateLimiter) -> Router<AppState> {
    let limited = Router::new()
        .route("/{provider}/login", get(login))
        .route("/{provider}/callback", get(callback))
        .route("/exchange", post(exchange))
        .route_layer(middleware::from_fn_with_state(limiter, rate_limit::enforce));

    limited
        .route("/providers", get(providers))
        .route("/identities", get(identities))
        .route("/{provider}/link", post(link))
        .route("/{provider}", delete(unlink))
}

fn parse_provider(name: &str) -> Result<Provider, OAuthError> {
    Provider::parse(name).ok_or(OAuthError::ProviderUnavailable)
}
