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
    #[error("administrator privileges are required")]
    AdminRequired,
}

impl From<SecurityError> for AppError {
    fn from(e: SecurityError) -> Self {
        let message = e.to_string();
        match e {
            SecurityError::AdminRequired => AppError::Forbidden(message),
            _ => AppError::Unauthorized(message),
        }
    }
}
