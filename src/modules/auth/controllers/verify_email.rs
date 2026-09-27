//! POST /auth/email/verify

use crate::{
    common::{dto::MessageResponse, error::AppResult, extractors::ValidatedJson},
    modules::auth::{dto::VerifyEmailDto, services::AuthService},
};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/email/verify",
    request_body = VerifyEmailDto,
    responses(
        (status = 200, description = "Email verified", body = MessageResponse),
        (status = 400, description = "Link is invalid, expired or already used"),
    ),
    tag = "auth"
)]
pub(crate) async fn verify(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<VerifyEmailDto>,
) -> AppResult<Json<MessageResponse>> {
    auth.email_verification.verify(dto).await?;
    Ok(Json(MessageResponse::new("Email verified.")))
}
