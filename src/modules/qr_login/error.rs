use crate::common::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum QrError {
    /// Unknown session, wrong secret, or not yours: all look the same to a caller.
    #[error("QR login session not found")]
    NotFound,
    #[error("this QR code has expired, ask the other device for a new one")]
    Expired,
    #[error("this QR code was already scanned by another account")]
    AlreadyScanned,
    #[error("the verification code does not match the one shown on the other device")]
    WrongCode,
    #[error("too many wrong codes, this QR login was cancelled")]
    TooManyAttempts,
    #[error("email address is not verified")]
    EmailNotVerified,
}

impl From<QrError> for AppError {
    fn from(e: QrError) -> Self {
        let message = e.to_string();
        match e {
            QrError::NotFound => AppError::NotFound,
            QrError::Expired | QrError::WrongCode | QrError::TooManyAttempts => {
                AppError::BadRequest(message)
            }
            QrError::AlreadyScanned => AppError::Conflict(message),
            QrError::EmailNotVerified => AppError::Forbidden(message),
        }
    }
}
