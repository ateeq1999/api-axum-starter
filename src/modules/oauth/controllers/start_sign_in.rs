//! GET /auth/oauth/{provider}/login

use super::parse_provider;
use crate::common::error::AppResult;
use crate::modules::oauth::{dto::LoginQuery, services::OAuthService};
use axum::{
    extract::{Path, Query, State},
    response::Redirect,
};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/v1/auth/oauth/{provider}/login",
    params(
        ("provider" = String, Path, description = "google | github"),
        LoginQuery,
    ),
    responses((status = 307, description = "Redirects the browser to the provider")),
    tag = "oauth"
)]
pub(crate) async fn login(
    State(oauth): State<Arc<OAuthService>>,
    Path(provider): Path<String>,
    Query(query): Query<LoginQuery>,
) -> AppResult<Redirect> {
    let url = oauth
        .login_url(parse_provider(&provider)?, query.redirect.as_deref())
        .await?;
    Ok(Redirect::to(&url))
}
