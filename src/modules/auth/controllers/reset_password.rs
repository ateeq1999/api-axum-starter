//! POST /auth/password/reset

use crate::{
    common::{dto::MessageResponse, error::AppResult, extractors::ValidatedJson},
    modules::auth::{dto::ResetPasswordDto, services::AuthService},
};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/password/reset",
    request_body = ResetPasswordDto,
    responses(
        (status = 200, description = "Password updated", body = MessageResponse),
        (status = 400, description = "Link invalid/expired, or the password is too weak"),
    ),
    tag = "auth"
)]
pub(crate) async fn reset(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<ResetPasswordDto>,
) -> AppResult<Json<MessageResponse>> {
    auth.password_reset.reset(dto).await?;
    Ok(Json(MessageResponse::new("Password updated.")))
}
