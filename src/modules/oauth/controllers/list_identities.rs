//! GET /auth/oauth/identities

use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::oauth::{dto::IdentityResponse, services::OAuthService};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/v1/auth/oauth/identities",
    responses((status = 200, description = "Providers linked to the caller's account", body = [IdentityResponse])),
    security(("bearer_auth" = [])),
    tag = "oauth"
)]
pub(crate) async fn identities(
    actor: SessionUser,
    State(oauth): State<Arc<OAuthService>>,
) -> AppResult<Json<Vec<IdentityResponse>>> {
    Ok(Json(oauth.identities(&actor).await?))
}
