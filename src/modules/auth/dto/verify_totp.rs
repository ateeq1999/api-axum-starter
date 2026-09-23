use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

/// `code` accepts either a live 6-digit authenticator code or an `XXXXX-XXXXX` recovery code.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct VerifyTotpDto {
    #[validate(length(min = 1, message = "is required"))]
    pub pending_token: String,
    #[validate(length(
        min = 6,
        max = 11,
        message = "must be an authenticator code or a recovery code"
    ))]
    pub code: String,
}
