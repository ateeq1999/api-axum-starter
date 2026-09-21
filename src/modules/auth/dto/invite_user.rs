use serde::Deserialize;
use validator::Validate;

use crate::common::security::Role;

/// Admin invites someone: the account is created and a "set your password" link is emailed.
#[derive(Debug, Deserialize, Validate)]
pub struct InviteUserDto {
    #[validate(email(message = "must be a valid email"))]
    pub email: String,
    #[validate(length(min = 1, max = 100, message = "must be 1-100 characters"))]
    pub display_name: Option<String>,
    pub role: Option<Role>,
}
