use crate::common::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("this sign-in provider is not available")]
    ProviderUnavailable,
    #[error("this sign-in link is invalid or has expired")]
    InvalidGrant,
    #[error("add a password or another sign-in method before removing this one")]
    LastSignInMethod,
    #[error("this account is not linked to that provider")]
    NotLinked,
    #[error("this account is disabled")]
    AccountDisabled,
    #[error("email address is not verified")]
    EmailNotVerified,
    /// The provider misbehaved or is unreachable. Details are logged, not shown.
    #[error("the sign-in provider could not be reached, try again")]
    Provider(String),
}

impl From<OAuthError> for AppError {
    fn from(e: OAuthError) -> Self {
        let message = e.to_string();
        match e {
            OAuthError::ProviderUnavailable | OAuthError::NotLinked => AppError::NotFound,
            OAuthError::InvalidGrant => AppError::BadRequest(message),
            OAuthError::LastSignInMethod => AppError::BadRequest(message),
            OAuthError::AccountDisabled | OAuthError::EmailNotVerified => {
                AppError::Forbidden(message)
            }
            OAuthError::Provider(detail) => {
                tracing::warn!(detail, "oauth provider error");
                AppError::BadRequest(message)
            }
        }
    }
}
