use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};

use crate::{
    common::{error::AppResult, extractors::ValidatedJson, security::AdminUser},
    modules::users::{
        dto::{CreateUserDto, UserResponse},
        services::UsersService,
    },
};

#[utoipa::path(
    post,
    path = "/api/v1/users",
    request_body = CreateUserDto,
    responses(
        (status = 201, description = "User created", body = UserResponse),
        (status = 409, description = "Email already registered"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn create(
    AdminUser(actor): AdminUser,
    State(users): State<Arc<UsersService>>,
    ValidatedJson(dto): ValidatedJson<CreateUserDto>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    Ok((StatusCode::CREATED, Json(users.create(&actor, dto).await?)))
}
