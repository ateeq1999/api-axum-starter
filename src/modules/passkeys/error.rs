use crate::common::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum PasskeyError {
    #[error("you can register at most {0} passkeys, remove one first")]
    LimitReached(usize),
    #[error("passkey registration failed, try again")]
    RegistrationFailed,
    #[error("this passkey is already registered")]
    AlreadyRegistered,
    /// Deliberately vague: it covers unknown credentials, bad signatures and expired challenges.
    #[error("passkey sign-in failed")]
    AuthenticationFailed,
    #[error("passkey not found")]
    NotFound,
    #[error("add a password or another sign-in method before removing this passkey")]
    LastSignInMethod,
    #[error("email address is not verified")]
    EmailNotVerified,
}

impl From<PasskeyError> for AppError {
    fn from(e: PasskeyError) -> Self {
        let message = e.to_string();
        match e {
            PasskeyError::LimitReached(_)
            | PasskeyError::RegistrationFailed
            | PasskeyError::AlreadyRegistered
            | PasskeyError::LastSignInMethod => AppError::BadRequest(message),
            PasskeyError::AuthenticationFailed => AppError::Unauthorized(message),
            PasskeyError::NotFound => AppError::NotFound,
            PasskeyError::EmailNotVerified => AppError::Forbidden(message),
        }
    }
}
