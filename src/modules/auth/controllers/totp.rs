use std::sync::Arc;

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};

use crate::{
    common::{
        dto::MessageResponse, error::AppResult, extractors::ValidatedJson, security::SessionUser,
    },
    modules::auth::{
        dto::{
            DisableTotpDto, EnableTotpDto, TokenResponse, TotpEnabledResponse, TotpSetupResponse,
            VerifyTotpDto,
        },
        services::AuthService,
    },
    state::AppState,
};

/// `POST /2fa/verify` completes a login, so it is public (the caller does not have a session
/// yet) but rate limited like the other pre-session auth endpoints.
pub fn rate_limited() -> Router<AppState> {
    Router::new().route("/2fa/verify", post(verify))
}

/// Managing your own two-factor setup needs an interactive session, same as any other
/// credential-management action.
pub fn authenticated() -> Router<AppState> {
    Router::new()
        .route("/2fa/setup", post(setup))
        .route("/2fa/enable", post(enable))
        .route("/2fa/disable", post(disable))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/2fa/setup",
    responses((status = 200, description = "Secret + QR code; not yet enforced until /2fa/enable confirms it", body = TotpSetupResponse)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn setup(
    SessionUser(actor): SessionUser,
    State(auth): State<Arc<AuthService>>,
) -> AppResult<Json<TotpSetupResponse>> {
    Ok(Json(auth.totp.begin_setup(&actor).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/2fa/enable",
    request_body = EnableTotpDto,
    responses(
        (status = 200, description = "Two-factor enabled; recovery codes shown exactly once", body = TotpEnabledResponse),
        (status = 400, description = "Wrong code, already enabled, or no setup in progress"),
    ),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn enable(
    SessionUser(actor): SessionUser,
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<EnableTotpDto>,
) -> AppResult<Json<TotpEnabledResponse>> {
    Ok(Json(auth.totp.confirm_setup(&actor, &dto.code).await?))
}

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

#[utoipa::path(
    post,
    path = "/api/v1/auth/2fa/verify",
    request_body = VerifyTotpDto,
    responses(
        (status = 200, description = "Completes the login", body = TokenResponse),
        (status = 401, description = "Wrong authenticator/recovery code"),
        (status = 400, description = "pending_token is invalid or expired"),
    ),
    tag = "auth"
)]
pub(crate) async fn verify(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<VerifyTotpDto>,
) -> AppResult<(StatusCode, Json<TokenResponse>)> {
    Ok((
        StatusCode::OK,
        Json(
            auth.totp
                .verify_login(&dto.pending_token, &dto.code)
                .await?,
        ),
    ))
}
