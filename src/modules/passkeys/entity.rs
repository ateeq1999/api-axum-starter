use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct PasskeyRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub credential_id: String,
    /// The `webauthn_rs::Passkey` (public key, signature counter, ...) as JSON.
    pub credential_json: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeKind {
    Registration,
    Authentication,
}

impl ChallengeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ChallengeKind::Registration => "registration",
            ChallengeKind::Authentication => "authentication",
        }
    }
}
