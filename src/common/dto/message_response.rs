use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    pub message: &'static str,
}

impl MessageResponse {
    pub const fn new(message: &'static str) -> Self {
        Self { message }
    }
}
