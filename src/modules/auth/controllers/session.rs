use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use validator::Validate;

use super::password;
use crate::{
    auth::{extractor::AuthUser, jwt},
    error::{AppError, AppResult},
    extract::ValidatedJson,
    state::AppState,
    users::{model::UserResponse, repository},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/me", get(me))
}

#[derive(Debug, Deserialize, Validate)]
pub struct Credentials {
    #[validate(email(message = "must be a valid email"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "must be 8-128 characters"))]
    pub password: String,
}

#[derive(Serialize)]
struct TokenResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
}

async fn register(
    State(state): State<AppState>,
    ValidatedJson(body): ValidatedJson<Credentials>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    let email = body.email.trim().to_lowercase();
    let hash = tokio::task::spawn_blocking(move || password::hash(&body.password))
        .await
        .map_err(|e| AppError::Internal(e.into()))??;
    let user = repository::create(&state.db, &email, &hash).await?;
    Ok((StatusCode::CREATED, Json(user.into())))
}

async fn login(
    State(state): State<AppState>,
    ValidatedJson(body): ValidatedJson<Credentials>,
) -> AppResult<Json<TokenResponse>> {
    let email = body.email.trim().to_lowercase();
    let invalid = || AppError::Unauthorized("invalid email or password");

    let user = repository::find_by_email(&state.db, &email)
        .await?
        .ok_or_else(invalid)?;

    let stored = user.password_hash.clone();
    let ok = tokio::task::spawn_blocking(move || password::verify(&body.password, &stored))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if !ok {
        return Err(invalid());
    }

    let ttl = state.config.jwt_ttl_secs;
    let access_token = jwt::issue(user.id, &state.config.jwt_secret, ttl)?;
    Ok(Json(TokenResponse {
        access_token,
        token_type: "Bearer",
        expires_in: ttl,
    }))
}

async fn me(
    State(state): State<AppState>,
    AuthUser(id): AuthUser,
) -> AppResult<Json<UserResponse>> {
    let user = repository::find_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(user.into()))
}
