//! POST /auth/invitations

use crate::{
    common::{error::AppResult, extractors::ValidatedJson, security::AdminUser},
    modules::{
        auth::{dto::InviteUserDto, services::AuthService},
        users::dto::UserResponse,
    },
};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/invitations",
    request_body = InviteUserDto,
    responses((status = 201, description = "Account created; a \"set your password\" link was emailed", body = UserResponse)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn invite(
    _admin: AdminUser,
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<InviteUserDto>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    Ok((
        StatusCode::CREATED,
        Json(auth.password_reset.invite(dto).await?),
    ))
}
