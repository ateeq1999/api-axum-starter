use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct EnableTotpDto {
    #[validate(length(
        equal = 6,
        message = "must be the 6-digit code from your authenticator app"
    ))]
    pub code: String,
}
