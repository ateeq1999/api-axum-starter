//! POST /auth/register

use crate::{
    common::{error::AppResult, extractors::ValidatedJson},
    modules::{
        auth::{dto::CredentialsDto, services::AuthService},
        users::dto::UserResponse,
    },
};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = CredentialsDto,
    responses(
        (status = 201, description = "Account created; a verification email was sent", body = UserResponse),
        (status = 409, description = "Email already registered"),
        (status = 400, description = "Password is too weak or has appeared in a known breach"),
    ),
    tag = "auth"
)]
pub(crate) async fn register(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<CredentialsDto>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    Ok((StatusCode::CREATED, Json(auth.session.register(dto).await?)))
}
