use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use super::{entity::OAuthIdentity, provider::Provider};

#[derive(Debug, Serialize, ToSchema)]
pub struct ProviderInfo {
    pub provider: Provider,
    pub name: &'static str,
    /// Navigate the browser here (relative to the API host) to sign in with this provider.
    pub login_url: String,
}

impl ProviderInfo {
    pub fn new(provider: Provider) -> Self {
        Self {
            provider,
            name: provider.display_name(),
            login_url: format!("/api/v1/auth/oauth/{}/login", provider.as_str()),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IdentityResponse {
    pub provider: Provider,
    pub email: Option<String>,
    pub linked_at: DateTime<Utc>,
}

impl From<OAuthIdentity> for IdentityResponse {
    fn from(i: OAuthIdentity) -> Self {
        Self {
            provider: i.provider,
            email: i.email,
            linked_at: i.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthorizeUrlResponse {
    pub authorize_url: String,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LoginQuery {
    /// Path in the frontend to return to after signing in, e.g. `/settings`.
    pub redirect: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    /// Set by the provider when the user cancelled or something went wrong.
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ExchangeDto {
    #[validate(length(min = 1, max = 256, message = "is required"))]
    pub code: String,
}
