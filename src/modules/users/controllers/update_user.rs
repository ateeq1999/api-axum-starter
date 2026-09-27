use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::{
    common::{error::AppResult, extractors::ValidatedJson, security::AdminUser},
    modules::users::{
        dto::{UpdateUserDto, UserResponse},
        services::UsersService,
    },
};

#[utoipa::path(
    patch,
    path = "/api/v1/users/{id}",
    params(("id" = Uuid, Path, description = "User id")),
    request_body = UpdateUserDto,
    responses(
        (status = 200, description = "Updated user", body = UserResponse),
        (status = 400, description = "Would lock the actor out or remove the last admin"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn update(
    AdminUser(actor): AdminUser,
    State(users): State<Arc<UsersService>>,
    Path(id): Path<Uuid>,
    ValidatedJson(dto): ValidatedJson<UpdateUserDto>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.update(&actor, id, dto).await?))
}
