//! GET /auth/oauth/{provider}/callback

use super::parse_provider;
use crate::modules::oauth::{
    dto::CallbackQuery,
    services::{CallbackOutcome, OAuthService},
};
use crate::{common::error::AppResult, config::Config};
use axum::{
    extract::{Path, Query, State},
    http::header::CACHE_CONTROL,
    response::{IntoResponse, Redirect, Response},
};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/v1/auth/oauth/{provider}/callback",
    params(
        ("provider" = String, Path, description = "google | github"),
        CallbackQuery,
    ),
    responses((status = 307, description = "Redirects to <frontend>/oauth/callback with a code, a linked marker, or an error")),
    tag = "oauth"
)]
pub(crate) async fn callback(
    State(oauth): State<Arc<OAuthService>>,
    State(config): State<Arc<Config>>,
    Path(provider): Path<String>,
    Query(query): Query<CallbackQuery>,
) -> AppResult<Response> {
    let outcome = oauth
        .handle_callback(parse_provider(&provider)?, query)
        .await;
    Ok((
        [(CACHE_CONTROL, "no-store")],
        frontend_redirect(&config.frontend.url, outcome),
    )
        .into_response())
}

/// Every outcome, success or failure, lands on one frontend route: `/oauth/callback`.
fn frontend_redirect(frontend_url: &str, outcome: CallbackOutcome) -> Redirect {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    match outcome {
        CallbackOutcome::LoggedIn { code, redirect } => {
            query
                .append_pair("code", &code)
                .append_pair("redirect", &redirect);
        }
        CallbackOutcome::Linked { provider, redirect } => {
            query
                .append_pair("linked", provider.as_str())
                .append_pair("redirect", &redirect);
        }
        CallbackOutcome::Failed(reason) => {
            query.append_pair("error", reason);
        }
    }
    Redirect::to(&format!("{frontend_url}/oauth/callback?{}", query.finish()))
}
