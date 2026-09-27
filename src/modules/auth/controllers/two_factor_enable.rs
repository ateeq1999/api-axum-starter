//! POST /auth/2fa/enable

use crate::{
    common::{error::AppResult, extractors::ValidatedJson, security::SessionUser},
    modules::auth::{
        dto::{EnableTotpDto, TotpEnabledResponse},
        services::AuthService,
    },
};
use axum::{Json, extract::State};
use std::sync::Arc;

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
