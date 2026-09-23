use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

/// One recorded administrative action. `details` is a small JSON blob describing what changed
/// (e.g. `{"role": {"from": "user", "to": "admin"}}`), kept as an opaque string here since its
/// shape varies per `action` — see `modules::audit_log::action` for the recorded action names.
#[derive(Debug, Clone, FromRow)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub action: String,
    pub target_user_id: Option<Uuid>,
    pub details: String,
    pub created_at: DateTime<Utc>,
}
