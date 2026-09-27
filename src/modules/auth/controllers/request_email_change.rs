//! POST /auth/email/change

use crate::{
    common::{
        dto::MessageResponse, error::AppResult, extractors::ValidatedJson, security::SessionUser,
    },
    modules::auth::{dto::ChangeEmailDto, services::AuthService},
};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/email/change",
    request_body = ChangeEmailDto,
    responses((status = 202, description = "Confirmation link sent to the new address", body = MessageResponse)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn request_change(
    SessionUser(actor): SessionUser,
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<ChangeEmailDto>,
) -> AppResult<(StatusCode, Json<MessageResponse>)> {
    auth.email_verification.request_change(&actor, dto).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(MessageResponse::new(
            "We sent a confirmation link to the new address.",
        )),
    ))
}
