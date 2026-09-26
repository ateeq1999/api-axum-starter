use crate::common::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("the upload is empty")]
    Empty,
    #[error("the file is too large: at most {0} MiB")]
    TooLarge(usize),
    #[error("the file type could not be recognized")]
    UnrecognizedType,
    #[error("files of type {0} are not accepted")]
    TypeNotAllowed(String),
    #[error("no such file")]
    NotFound,
}

impl From<MediaError> for AppError {
    fn from(e: MediaError) -> Self {
        let message = e.to_string();
        match e {
            MediaError::Empty => AppError::BadRequest(message),
            MediaError::TooLarge(_) => AppError::PayloadTooLarge(message),
            MediaError::UnrecognizedType | MediaError::TypeNotAllowed(_) => {
                AppError::UnsupportedMediaType(message)
            }
            MediaError::NotFound => AppError::NotFound,
        }
    }
}
