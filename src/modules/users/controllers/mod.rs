//! HTTP handlers, one file per endpoint. Each one unpacks the request and calls one
//! `UsersService` method.

use axum::{Router, routing::get};

use crate::state::AppState;

pub mod create_user;
pub mod delete_user;
pub mod get_current_user;
pub mod get_user;
pub mod list_users;
pub mod update_profile;
pub mod update_user;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_users::list).post(create_user::create))
        .route(
            "/me",
            get(get_current_user::me).patch(update_profile::update_me),
        )
        .route(
            "/{id}",
            get(get_user::get_one)
                .patch(update_user::update)
                .delete(delete_user::remove),
        )
}
