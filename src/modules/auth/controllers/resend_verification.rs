//! POST /auth/email/verification/resend

use crate::{
    common::{dto::MessageResponse, error::AppResult, security::AuthUser},
    modules::auth::services::AuthService,
};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/email/verification/resend",
    responses((status = 202, description = "Accepted; a no-op if already verified", body = MessageResponse)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn resend(
    actor: AuthUser,
    State(auth): State<Arc<AuthService>>,
) -> AppResult<(StatusCode, Json<MessageResponse>)> {
    auth.email_verification.resend(&actor).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(MessageResponse::new(
            "If your email is not verified yet, we sent a new link.",
        )),
    ))
}
