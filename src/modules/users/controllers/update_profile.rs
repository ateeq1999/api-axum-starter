use std::sync::Arc;

use axum::{Json, extract::State};

use crate::{
    common::{error::AppResult, extractors::ValidatedJson, security::AuthUser},
    modules::users::{
        dto::{UpdateProfileDto, UserResponse},
        services::UsersService,
    },
};

#[utoipa::path(
    patch,
    path = "/api/v1/users/me",
    request_body = UpdateProfileDto,
    responses((status = 200, description = "Updated profile", body = UserResponse)),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn update_me(
    actor: AuthUser,
    State(users): State<Arc<UsersService>>,
    ValidatedJson(dto): ValidatedJson<UpdateProfileDto>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.update_profile(&actor, dto).await?))
}
