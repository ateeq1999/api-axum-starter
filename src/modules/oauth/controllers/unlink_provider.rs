//! DELETE /auth/oauth/{provider}

use super::parse_provider;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::oauth::services::OAuthService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use std::sync::Arc;

#[utoipa::path(
    delete,
    path = "/api/v1/auth/oauth/{provider}",
    params(("provider" = String, Path, description = "google | github")),
    responses(
        (status = 204, description = "Unlinked"),
        (status = 400, description = "Not linked, or the account's last sign-in method"),
    ),
    security(("bearer_auth" = [])),
    tag = "oauth"
)]
pub(crate) async fn unlink(
    actor: SessionUser,
    State(oauth): State<Arc<OAuthService>>,
    Path(provider): Path<String>,
) -> AppResult<StatusCode> {
    oauth.unlink(&actor, parse_provider(&provider)?).await?;
    Ok(StatusCode::NO_CONTENT)
}
