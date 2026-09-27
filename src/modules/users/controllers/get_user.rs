use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::users::{dto::UserResponse, services::UsersService},
};

#[utoipa::path(
    get,
    path = "/api/v1/users/{id}",
    params(("id" = Uuid, Path, description = "User id")),
    responses(
        (status = 200, description = "The requested user", body = UserResponse),
        (status = 403, description = "Not allowed to view this user"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn get_one(
    actor: AuthUser,
    State(users): State<Arc<UsersService>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.get(&actor, id).await?))
}
