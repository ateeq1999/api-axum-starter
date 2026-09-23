use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

/// Requires the current password so a stolen session token alone cannot turn off two-factor.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct DisableTotpDto {
    #[validate(length(min = 8, max = 128, message = "must be 8-128 characters"))]
    pub current_password: String,
}
