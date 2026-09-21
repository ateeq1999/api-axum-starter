use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct ForgotPasswordDto {
    #[validate(email(message = "must be a valid email"))]
    pub email: String,
}
