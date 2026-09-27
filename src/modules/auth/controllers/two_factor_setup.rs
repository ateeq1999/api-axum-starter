//! POST /auth/2fa/setup

use crate::{
    common::{error::AppResult, security::SessionUser},
    modules::auth::{dto::TotpSetupResponse, services::AuthService},
};
use axum::{Json, extract::State};
use std::sync::Arc;

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
