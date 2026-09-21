use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use serde::Deserialize;
use validator::Validate;

use super::password;
use crate::{
    error::{AppError, AppResult},
    extract::ValidatedJson,
    state::AppState,
    users::{model::UserResponse, repository},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/register", post(register))
}

#[derive(Debug, Deserialize, Validate)]
pub struct Credentials {
    #[validate(email(message = "must be a valid email"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "must be 8-128 characters"))]
    pub password: String,
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
