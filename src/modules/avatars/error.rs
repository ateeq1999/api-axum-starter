use crate::common::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum AvatarError {
    #[error("the file is not a supported image (use PNG, JPEG, WebP or GIF)")]
    InvalidImage,
    #[error("the image is too large: at most {0} MiB and 8192 x 8192 pixels")]
    TooLarge(usize),
    #[error("no such avatar")]
    NotFound,
}

impl From<AvatarError> for AppError {
    fn from(e: AvatarError) -> Self {
        let message = e.to_string();
        match e {
            AvatarError::InvalidImage => AppError::BadRequest(message),
            AvatarError::TooLarge(_) => AppError::PayloadTooLarge(message),
            AvatarError::NotFound => AppError::NotFound,
        }
    }
}
