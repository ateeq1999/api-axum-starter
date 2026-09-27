//! POST /auth/2fa/verify

use crate::{
    common::{error::AppResult, extractors::ValidatedJson},
    modules::auth::{
        dto::{TokenResponse, VerifyTotpDto},
        services::AuthService,
    },
};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

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
