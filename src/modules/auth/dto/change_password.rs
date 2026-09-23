use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ChangePasswordDto {
    #[validate(length(min = 1, max = 128, message = "is required"))]
    pub current_password: String,
    #[validate(length(min = 8, max = 128, message = "must be 8-128 characters"))]
    pub new_password: String,
}
