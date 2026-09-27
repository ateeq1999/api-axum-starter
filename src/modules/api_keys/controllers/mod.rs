use axum::{
    Router,
    routing::{delete, get},
};

use crate::state::AppState;

pub mod create_key;
pub mod list_keys;
pub mod revoke_key;

use create_key::create;
use list_keys::list;
use revoke_key::revoke;

/// API keys are managed with an interactive sign-in only: a key cannot mint or revoke keys.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", delete(revoke))
}
