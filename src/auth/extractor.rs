use axum::{extract::FromRequestParts, http::request::Parts};
use uuid::Uuid;

use super::jwt;
use crate::{error::AppError, state::AppState};

#[derive(Debug, Clone, Copy)]
pub struct AuthUser(pub Uuid);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized("missing Authorization header"))?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or(AppError::Unauthorized("expected a Bearer token"))?;

        let claims = jwt::verify(token, &state.config.jwt_secret)?;
        Ok(AuthUser(claims.sub))
    }
}
