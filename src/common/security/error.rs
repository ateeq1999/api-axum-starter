use crate::common::error::AppError;

/// Named failure reasons for authentication/authorization guards.
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("missing Authorization header")]
    MissingAuthorizationHeader,
    #[error("expected a Bearer token")]
    MalformedBearerToken,
    #[error("token is invalid or expired")]
    InvalidToken,
    #[error("API key is invalid, expired or revoked")]
    InvalidApiKey,
    #[error("this API key is read-only")]
    ApiKeyReadOnly,
    #[error("this action needs an interactive sign-in, an API key is not accepted")]
    SessionRequired,
    #[error("administrator privileges are required")]
    AdminRequired,
}

impl From<SecurityError> for AppError {
    fn from(e: SecurityError) -> Self {
        let message = e.to_string();
        match e {
            SecurityError::AdminRequired
            | SecurityError::ApiKeyReadOnly
            | SecurityError::SessionRequired => AppError::Forbidden(message),
            _ => AppError::Unauthorized(message),
        }
    }
}
