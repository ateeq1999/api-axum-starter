use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};

use crate::{
    common::{error::AppResult, extractors::ValidatedJson, security::AuthUser},
    modules::{
        auth::{
            dto::{CredentialsDto, TokenResponse},
            services::AuthService,
        },
        users::dto::UserResponse,
    },
    state::AppState,
};

/// Public routes that are worth rate limiting.
pub fn rate_limited() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
}

pub fn authenticated() -> Router<AppState> {
    Router::new().route("/me", get(me))
}

async fn register(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<CredentialsDto>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    Ok((StatusCode::CREATED, Json(auth.session.register(dto).await?)))
}

async fn login(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<CredentialsDto>,
) -> AppResult<Json<TokenResponse>> {
    Ok(Json(auth.session.login(dto).await?))
}

async fn me(
    actor: AuthUser,
    State(auth): State<Arc<AuthService>>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(auth.session.me(&actor).await?))
}
