use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

/// Body for redeeming a link token: email verification and email-change confirmation.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct VerifyEmailDto {
    #[validate(length(min = 1, max = 256, message = "is required"))]
    pub token: String,
}
