/// Mail failures are logged, never returned to API clients.
#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("invalid mail configuration: {0}")]
    Config(String),
    #[error("could not render email template: {0}")]
    Render(#[from] askama::Error),
    #[error("could not build email message: {0}")]
    Build(String),
    #[error("smtp delivery failed (permanent: {permanent}): {reason}")]
    Delivery { permanent: bool, reason: String },
}

impl MailError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Delivery {
                permanent: false,
                ..
            }
        )
    }
}
