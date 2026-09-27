use axum::{
    Router,
    routing::{get, put},
};

use crate::state::AppState;

pub mod remove_avatar;
pub mod serve_avatar;
pub mod set_avatar;

use remove_avatar::remove;
use serve_avatar::serve;
use set_avatar::upload;

/// `PUT /users/me/avatar` (raw image bytes as the body) and `DELETE /users/me/avatar`.
pub fn own_router() -> Router<AppState> {
    Router::new().route("/", put(upload).delete(remove))
}

/// `GET /avatars/{file}`: public, so `<img src>` works without an Authorization header.
/// The file name is an unguessable random id that changes on every upload.
pub fn public_router() -> Router<AppState> {
    Router::new().route("/{file}", get(serve))
}
