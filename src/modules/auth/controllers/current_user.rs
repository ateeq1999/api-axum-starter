//! GET /auth/me

use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::{auth::services::AuthService, users::dto::UserResponse},
};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses((status = 200, description = "The signed-in user", body = UserResponse)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn me(
    actor: AuthUser,
    State(auth): State<Arc<AuthService>>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(auth.session.me(&actor).await?))
}
