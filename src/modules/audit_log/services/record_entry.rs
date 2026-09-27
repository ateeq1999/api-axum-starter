//! Recording an administrative action. Never fails the action itself.

use super::AuditLogService;
use serde::Serialize;
use uuid::Uuid;

impl AuditLogService {
    /// Records one administrative action. `details` is serialized to JSON; failures to record
    /// are logged, not propagated — an audit-logging hiccup must never fail the action itself
    /// (e.g. block an admin from deactivating a compromised account).
    pub async fn record(
        &self,
        actor_user_id: Uuid,
        action: &'static str,
        target_user_id: Option<Uuid>,
        details: impl Serialize,
    ) {
        let details_json = match serde_json::to_string(&details) {
            Ok(json) => json,
            Err(error) => {
                tracing::error!(%error, action, "could not serialize audit log details");
                return;
            }
        };
        if let Err(error) = self
            .repo
            .record(actor_user_id, action, target_user_id, &details_json)
            .await
        {
            tracing::error!(%error, action, "could not record audit log entry");
        }
    }
}
