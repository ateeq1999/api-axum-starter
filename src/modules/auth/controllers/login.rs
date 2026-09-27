//! POST /auth/login

use crate::{
    common::{error::AppResult, extractors::ValidatedJson},
    modules::auth::{
        dto::{CredentialsDto, LoginResponse},
        services::AuthService,
    },
};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = CredentialsDto,
    responses(
        (status = 200, description = "Either a session (access_token) or, for a two-factor-enabled account, requires_totp + pending_token", body = LoginResponse),
        (status = 401, description = "Invalid email/password, or the account is temporarily locked"),
        (status = 403, description = "Account disabled or email not verified"),
    ),
    tag = "auth"
)]
pub(crate) async fn login(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<CredentialsDto>,
) -> AppResult<Json<LoginResponse>> {
    Ok(Json(auth.session.login(dto).await?))
}
