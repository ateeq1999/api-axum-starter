use crate::common::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid email or password")]
    InvalidCredentials,
    #[error("this link is invalid or has expired")]
    InvalidOrExpiredLink,
    #[error("current password is incorrect")]
    WrongCurrentPassword,
    #[error("the new email address is the same as the current one")]
    EmailUnchanged,
    #[error("email address is not verified")]
    EmailNotVerified,
    #[error("this account is disabled")]
    AccountDisabled,
    #[error("two-factor authentication is already enabled")]
    TotpAlreadyEnabled,
    #[error("start two-factor setup first")]
    NoTotpSetupInProgress,
    #[error("that code is incorrect or has expired")]
    InvalidTotpCode,
}

impl From<AuthError> for AppError {
    fn from(e: AuthError) -> Self {
        let message = e.to_string();
        match e {
            AuthError::InvalidOrExpiredLink
            | AuthError::WrongCurrentPassword
            | AuthError::EmailUnchanged
            | AuthError::TotpAlreadyEnabled
            | AuthError::NoTotpSetupInProgress => AppError::BadRequest(message),
            AuthError::EmailNotVerified | AuthError::AccountDisabled => {
                AppError::Forbidden(message)
            }
            AuthError::InvalidCredentials | AuthError::InvalidTotpCode => {
                AppError::Unauthorized(message)
            }
        }
    }
}
