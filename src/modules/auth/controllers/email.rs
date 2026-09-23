use std::sync::Arc;

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};

use crate::{
    common::{
        dto::MessageResponse,
        error::AppResult,
        extractors::ValidatedJson,
        security::{AuthUser, SessionUser},
    },
    modules::auth::{
        dto::{ChangeEmailDto, VerifyEmailDto},
        services::AuthService,
    },
    state::AppState,
};

/// Token redemption and resend are abusable, so they share the limiter.
pub fn rate_limited() -> Router<AppState> {
    Router::new()
        .route("/email/verify", post(verify))
        .route("/email/verification/resend", post(resend))
        .route("/email/change/confirm", post(confirm_change))
}

pub fn authenticated() -> Router<AppState> {
    Router::new().route("/email/change", post(request_change))
}

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
