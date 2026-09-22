use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

/// pending -> scanned -> approved -> consumed, or -> rejected at any point before consumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum QrStatus {
    /// Shown on the new device, nobody has scanned it yet.
    Pending,
    /// A signed-in device scanned it and is asking its user to confirm.
    Scanned,
    /// The user confirmed; the new device can collect its access token (once).
    Approved,
    Rejected,
    /// The new device already collected its token.
    Consumed,
}

impl QrStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            QrStatus::Pending => "pending",
            QrStatus::Scanned => "scanned",
            QrStatus::Approved => "approved",
            QrStatus::Rejected => "rejected",
            QrStatus::Consumed => "consumed",
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct QrSession {
    pub id: String,
    pub secret_hash: String,
    pub code: String,
    pub status: QrStatus,
    pub user_id: Option<Uuid>,
    pub requester_ip: Option<String>,
    pub requester_agent: Option<String>,
    pub attempts: i64,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
