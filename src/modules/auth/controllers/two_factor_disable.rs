//! POST /auth/2fa/disable

use crate::{
    common::{
        dto::MessageResponse, error::AppResult, extractors::ValidatedJson, security::SessionUser,
    },
    modules::auth::{dto::DisableTotpDto, services::AuthService},
};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/2fa/disable",
    request_body = DisableTotpDto,
    responses(
        (status = 200, description = "Two-factor disabled", body = MessageResponse),
        (status = 400, description = "Wrong current password"),
    ),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn disable(
    SessionUser(actor): SessionUser,
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<DisableTotpDto>,
) -> AppResult<Json<MessageResponse>> {
    auth.totp.disable(&actor, dto.current_password).await?;
    Ok(Json(MessageResponse::new(
        "Two-factor authentication disabled.",
    )))
}
