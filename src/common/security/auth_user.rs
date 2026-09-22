use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{HeaderMap, header::AUTHORIZATION, request::Parts},
};
use uuid::Uuid;

use super::{
    api_key::{API_KEY_PREFIX, ApiKeyAuth, ApiKeyScope, Credential},
    error::SecurityError,
    jwt,
    jwt::JwtSettings,
    role::Role,
};
use crate::common::error::AppError;

/// The authenticated caller: a JWT session or an API key.
#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub id: Uuid,
    pub role: Role,
    pub credential: Credential,
}

impl AuthUser {
    pub fn session(id: Uuid, role: Role) -> Self {
        Self {
            id,
            role,
            credential: Credential::Session,
        }
    }
}

const X_API_KEY: &str = "x-api-key";

/// `X-API-Key: ak_...` or `Authorization: Bearer <jwt or ak_...>`.
fn presented_credential(headers: &HeaderMap) -> Result<&str, SecurityError> {
    if let Some(key) = headers.get(X_API_KEY).and_then(|v| v.to_str().ok()) {
        return Ok(key.trim());
    }
    let header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(SecurityError::MissingAuthorizationHeader)?;
    header
        .strip_prefix("Bearer ")
        .ok_or(SecurityError::MalformedBearerToken)
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Arc<JwtSettings>: FromRef<S>,
    ApiKeyAuth: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let presented = presented_credential(&parts.headers)?;

        if presented.starts_with(API_KEY_PREFIX) {
            let user = ApiKeyAuth::from_ref(state).0.verify(presented).await?;
            let read_only = user.credential == Credential::ApiKey(ApiKeyScope::Read);
            let safe_method = parts.method.is_safe();
            if read_only && !safe_method {
                return Err(SecurityError::ApiKeyReadOnly.into());
            }
            return Ok(user);
        }

        let settings = <Arc<JwtSettings>>::from_ref(state);
        let claims = jwt::verify(presented, &settings.secret)?;
        Ok(AuthUser::session(claims.sub, claims.role))
    }
}
