use std::sync::Arc;

use axum::{Json, extract::State};

use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::users::{dto::UserResponse, services::UsersService},
};

#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    responses((status = 200, description = "The signed-in user", body = UserResponse)),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn me(
    actor: AuthUser,
    State(users): State<Arc<UsersService>>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.get(&actor, actor.id).await?))
}
