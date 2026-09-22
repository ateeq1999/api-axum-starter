use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};

use super::{api_key::ApiKeyAuth, auth_user::AuthUser, error::SecurityError, jwt::JwtSettings};
use crate::common::error::AppError;

/// Like [`AuthUser`], but rejects callers that are not administrators (403).
#[derive(Debug, Clone, Copy)]
pub struct AdminUser(pub AuthUser);

impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
    Arc<JwtSettings>: FromRef<S>,
    ApiKeyAuth: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if user.role.is_admin() {
            Ok(Self(user))
        } else {
            Err(SecurityError::AdminRequired.into())
        }
    }
}
