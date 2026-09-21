use serde::Deserialize;
use validator::Validate;

use crate::common::security::Role;

/// Admin PATCH: every field is optional, absent means "leave unchanged".
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserDto {
    #[validate(length(min = 1, max = 100, message = "must be 1-100 characters"))]
    pub display_name: Option<String>,
    pub role: Option<Role>,
    pub is_active: Option<bool>,
}
