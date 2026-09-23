use crate::common::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum UsersError {
    #[error("email is already registered")]
    EmailTaken,
    #[error("user not found")]
    NotFound,
    #[error("you are not allowed to access this user")]
    Forbidden,
    #[error("you cannot delete your own account here")]
    CannotDeleteSelf,
    #[error("administrators cannot demote or deactivate themselves")]
    CannotLockOutSelf,
    #[error("at least one active administrator must remain")]
    CannotRemoveLastAdmin,
    #[error("{0}")]
    PasswordTooWeak(String),
    #[error("this password has appeared in a known data breach; choose a different one")]
    PasswordPreviouslyBreached,
}

impl From<UsersError> for AppError {
    fn from(e: UsersError) -> Self {
        let message = e.to_string();
        match e {
            UsersError::EmailTaken => AppError::Conflict(message),
            UsersError::NotFound => AppError::NotFound,
            UsersError::Forbidden => AppError::Forbidden(message),
            UsersError::CannotDeleteSelf
            | UsersError::CannotLockOutSelf
            | UsersError::CannotRemoveLastAdmin
            | UsersError::PasswordTooWeak(_)
            | UsersError::PasswordPreviouslyBreached => AppError::BadRequest(message),
        }
    }
}
