use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ForgotPasswordDto {
    #[validate(email(message = "must be a valid email"))]
    pub email: String,
}
