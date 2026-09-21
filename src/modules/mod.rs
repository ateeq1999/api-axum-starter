pub mod auth;
pub mod health;
pub mod mail;
pub mod users;

use axum::Router;

use crate::state::AppState;

/// Feature routes, to be nested under the API prefix.
pub fn router(state: &AppState) -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router(state.rate_limiter.clone()))
        .nest("/users", users::router())
}
