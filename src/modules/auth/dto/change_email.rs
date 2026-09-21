use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct ChangeEmailDto {
    #[validate(email(message = "must be a valid email"))]
    pub new_email: String,
    #[validate(length(min = 1, max = 128, message = "is required"))]
    pub current_password: String,
}
