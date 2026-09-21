use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};
use uuid::Uuid;

use super::{error::SecurityError, jwt, jwt::JwtSettings, role::Role};
use crate::common::error::AppError;

/// The authenticated caller, extracted from the `Authorization: Bearer` header.
#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub id: Uuid,
    pub role: Role,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Arc<JwtSettings>: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let settings = <Arc<JwtSettings>>::from_ref(state);

        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(SecurityError::MissingAuthorizationHeader)?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or(SecurityError::MalformedBearerToken)?;

        let claims = jwt::verify(token, &settings.secret)?;
        Ok(AuthUser {
            id: claims.sub,
            role: claims.role,
        })
    }
}
