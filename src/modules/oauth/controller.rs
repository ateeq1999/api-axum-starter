use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header::CACHE_CONTROL},
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, post},
};

use super::{
    dto::{
        AuthorizeUrlResponse, CallbackQuery, ExchangeDto, IdentityResponse, LoginQuery,
        ProviderInfo,
    },
    error::OAuthError,
    provider::Provider,
    service::{CallbackOutcome, OAuthService},
};
use crate::{
    common::{
        error::AppResult,
        extractors::ValidatedJson,
        middleware::rate_limit::{self, RateLimiter},
        security::SessionUser,
    },
    config::Config,
    modules::auth::dto::TokenResponse,
    state::AppState,
};

/// Mounted at `/api/v1/auth/oauth`.
///
/// Sign-in flow: the SPA sends the browser to `GET /{provider}/login`; the provider redirects
/// to `GET /{provider}/callback`, which redirects on to `<frontend>/oauth/callback?code=...`;
/// the SPA then posts that code to `POST /exchange` to get an access token.
pub fn router(limiter: RateLimiter) -> Router<AppState> {
    let limited = Router::new()
        .route("/{provider}/login", get(login))
        .route("/{provider}/callback", get(callback))
        .route("/exchange", post(exchange))
        .route_layer(middleware::from_fn_with_state(limiter, rate_limit::enforce));

    limited
        .route("/providers", get(providers))
        .route("/identities", get(identities))
        .route("/{provider}/link", post(link))
        .route("/{provider}", delete(unlink))
}

fn parse_provider(name: &str) -> Result<Provider, OAuthError> {
    Provider::parse(name).ok_or(OAuthError::ProviderUnavailable)
}

async fn providers(State(oauth): State<Arc<OAuthService>>) -> Json<Vec<ProviderInfo>> {
    Json(oauth.providers())
}

async fn login(
    State(oauth): State<Arc<OAuthService>>,
    Path(provider): Path<String>,
    Query(query): Query<LoginQuery>,
) -> AppResult<Redirect> {
    let url = oauth
        .login_url(parse_provider(&provider)?, query.redirect.as_deref())
        .await?;
    Ok(Redirect::to(&url))
}

async fn callback(
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

async fn exchange(
    State(oauth): State<Arc<OAuthService>>,
    ValidatedJson(dto): ValidatedJson<ExchangeDto>,
) -> AppResult<Json<TokenResponse>> {
    Ok(Json(oauth.exchange(&dto.code).await?))
}

async fn link(
    actor: SessionUser,
    State(oauth): State<Arc<OAuthService>>,
    Path(provider): Path<String>,
) -> AppResult<Json<AuthorizeUrlResponse>> {
    Ok(Json(
        oauth.link_url(&actor, parse_provider(&provider)?).await?,
    ))
}

async fn identities(
    actor: SessionUser,
    State(oauth): State<Arc<OAuthService>>,
) -> AppResult<Json<Vec<IdentityResponse>>> {
    Ok(Json(oauth.identities(&actor).await?))
}

async fn unlink(
    actor: SessionUser,
    State(oauth): State<Arc<OAuthService>>,
    Path(provider): Path<String>,
) -> AppResult<StatusCode> {
    oauth.unlink(&actor, parse_provider(&provider)?).await?;
    Ok(StatusCode::NO_CONTENT)
}
