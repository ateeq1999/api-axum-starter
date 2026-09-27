//! POST /auth/oauth/{provider}/link

use super::parse_provider;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::oauth::{dto::AuthorizeUrlResponse, services::OAuthService};
use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/oauth/{provider}/link",
    params(("provider" = String, Path, description = "google | github")),
    responses((status = 200, description = "Where to send the browser to link this provider", body = AuthorizeUrlResponse)),
    security(("bearer_auth" = [])),
    tag = "oauth"
)]
pub(crate) async fn link(
    actor: SessionUser,
    State(oauth): State<Arc<OAuthService>>,
    Path(provider): Path<String>,
) -> AppResult<Json<AuthorizeUrlResponse>> {
    Ok(Json(
        oauth.link_url(&actor, parse_provider(&provider)?).await?,
    ))
}
