use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::provider::Provider;

#[derive(Debug, Clone, FromRow)]
pub struct OAuthIdentity {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: Provider,
    pub provider_user_id: String,
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// An authorization request waiting for the provider to redirect back.
#[derive(Debug, Clone, FromRow)]
pub struct OAuthState {
    pub provider: Provider,
    pub pkce_verifier: String,
    /// Set when an already signed-in user is linking a provider to their account.
    pub user_id: Option<Uuid>,
    pub redirect_path: String,
}

/// What we learn about the person from the provider.
#[derive(Debug, Clone)]
pub struct ProviderProfile {
    pub provider_user_id: String,
    pub email: Option<String>,
    /// The provider vouches that the person controls `email`.
    pub email_verified: bool,
    pub name: Option<String>,
}
