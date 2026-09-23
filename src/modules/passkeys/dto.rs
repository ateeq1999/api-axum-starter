use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

use super::entity::PasskeyRow;

/// First half of a ceremony: `options` goes to `navigator.credentials.create()` / `.get()`
/// (via `PublicKeyCredential.parseCreationOptionsFromJSON` / `parseRequestOptionsFromJSON`),
/// and `challenge_id` comes back with the browser's answer.
#[derive(Debug, Serialize)]
pub struct BeginResponse<T> {
    pub challenge_id: String,
    pub options: T,
}

#[derive(Debug, Deserialize, Validate)]
pub struct FinishRegistrationDto {
    #[validate(length(min = 1, max = 100))]
    pub challenge_id: String,
    /// Shown in the passkey list, e.g. "MacBook Touch ID". Defaults to "Passkey".
    #[validate(length(min = 1, max = 100, message = "must be 1-100 characters"))]
    pub name: Option<String>,
    /// `credential.toJSON()` from the browser.
    pub credential: RegisterPublicKeyCredential,
}

#[derive(Debug, Deserialize, Validate)]
pub struct FinishLoginDto {
    #[validate(length(min = 1, max = 100))]
    pub challenge_id: String,
    /// `credential.toJSON()` from the browser.
    pub credential: PublicKeyCredential,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PasskeyResponse {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl From<PasskeyRow> for PasskeyResponse {
    fn from(p: PasskeyRow) -> Self {
        Self {
            id: p.id,
            name: p.name,
            created_at: p.created_at,
            last_used_at: p.last_used_at,
        }
    }
}
