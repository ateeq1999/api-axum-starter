use crate::common::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum ApiKeysError {
    #[error("you can have at most {0} active API keys, revoke one first")]
    LimitReached(i64),
    #[error("API key not found")]
    NotFound,
}

impl From<ApiKeysError> for AppError {
    fn from(e: ApiKeysError) -> Self {
        let message = e.to_string();
        match e {
            ApiKeysError::LimitReached(_) => AppError::BadRequest(message),
            ApiKeysError::NotFound => AppError::NotFound,
        }
    }
}
