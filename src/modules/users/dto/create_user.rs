use serde::Deserialize;
use validator::Validate;

use crate::common::security::Role;

/// Admin creates an account with a password they choose.
/// To let the person pick their own password, use `POST /auth/invitations` instead.
#[derive(Debug, Deserialize, Validate)]
pub struct CreateUserDto {
    #[validate(email(message = "must be a valid email"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "must be 8-128 characters"))]
    pub password: String,
    #[validate(length(min = 1, max = 100, message = "must be 1-100 characters"))]
    pub display_name: Option<String>,
    pub role: Option<Role>,
}
