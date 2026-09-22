use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};

use super::{
    api_key::{ApiKeyAuth, Credential},
    auth_user::AuthUser,
    error::SecurityError,
    jwt::JwtSettings,
};
use crate::common::error::AppError;

/// Like [`AuthUser`], but only an interactive sign-in (JWT) is accepted, never an API key.
/// Used for anything that manages credentials: passwords, emails, API keys, passkeys, OAuth
/// links and approving QR logins. A leaked API key therefore cannot be used to take an account over.
#[derive(Debug, Clone, Copy)]
pub struct SessionUser(pub AuthUser);

impl<S> FromRequestParts<S> for SessionUser
where
    S: Send + Sync,
    Arc<JwtSettings>: FromRef<S>,
    ApiKeyAuth: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if user.credential == Credential::Session {
            Ok(Self(user))
        } else {
            Err(SecurityError::SessionRequired.into())
        }
    }
}
