use serde::Deserialize;
use validator::Validate;

use crate::common::security::ApiKeyScope;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateApiKeyDto {
    #[validate(length(min = 1, max = 100, message = "must be 1-100 characters"))]
    pub name: String,
    /// `read` (default): safe HTTP methods only. `write`: everything an API key may do.
    pub scope: Option<ApiKeyScope>,
    /// Omit for a key that never expires.
    #[validate(range(min = 1, max = 3650, message = "must be 1-3650"))]
    pub expires_in_days: Option<u32>,
}
