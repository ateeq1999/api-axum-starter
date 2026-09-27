//! POST /auth/email/change/confirm

use crate::{
    common::{dto::MessageResponse, error::AppResult, extractors::ValidatedJson},
    modules::auth::{dto::VerifyEmailDto, services::AuthService},
};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/email/change/confirm",
    request_body = VerifyEmailDto,
    responses(
        (status = 200, description = "Email address updated", body = MessageResponse),
        (status = 400, description = "Link is invalid, expired or already used"),
    ),
    tag = "auth"
)]
pub(crate) async fn confirm_change(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<VerifyEmailDto>,
) -> AppResult<Json<MessageResponse>> {
    auth.email_verification.confirm_change(dto).await?;
    Ok(Json(MessageResponse::new("Email address updated.")))
}
