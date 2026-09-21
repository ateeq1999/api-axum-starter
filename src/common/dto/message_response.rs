use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: &'static str,
}

impl MessageResponse {
    pub const fn new(message: &'static str) -> Self {
        Self { message }
    }
}
