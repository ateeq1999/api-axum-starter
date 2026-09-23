use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::modules::audit_log::entity::AuditLogEntry;

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditLogEntryResponse {
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub action: String,
    pub target_user_id: Option<Uuid>,
    /// Parsed from the stored JSON string; falls back to `null` if a stored row is ever
    /// unreadable, rather than failing the whole listing over one bad entry.
    pub details: Value,
    pub created_at: DateTime<Utc>,
}

impl From<AuditLogEntry> for AuditLogEntryResponse {
    fn from(entry: AuditLogEntry) -> Self {
        Self {
            id: entry.id,
            actor_user_id: entry.actor_user_id,
            action: entry.action,
            target_user_id: entry.target_user_id,
            details: serde_json::from_str(&entry.details).unwrap_or(Value::Null),
            created_at: entry.created_at,
        }
    }
}
