//! GET /auth/oauth/providers

use crate::modules::oauth::{dto::ProviderInfo, services::OAuthService};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/v1/auth/oauth/providers",
    responses((status = 200, description = "Configured providers only", body = [ProviderInfo])),
    tag = "oauth"
)]
pub(crate) async fn providers(State(oauth): State<Arc<OAuthService>>) -> Json<Vec<ProviderInfo>> {
    Json(oauth.providers())
}
