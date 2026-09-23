use serde::Serialize;
use uuid::Uuid;

use super::{
    dto::{AuditLogEntryResponse, ListAuditLogQuery},
    repository::AuditLogRepository,
};
use crate::common::{dto::PaginatedResponse, error::AppResult};

/// Recorded action names. Kept as constants (rather than a free-form string at each call site) so
/// a typo cannot silently create a new, undiscoverable action name.
pub mod action {
    pub const USER_CREATED_BY_ADMIN: &str = "user.created_by_admin";
    pub const USER_ROLE_CHANGED: &str = "user.role_changed";
    pub const USER_ACTIVE_STATUS_CHANGED: &str = "user.active_status_changed";
    pub const USER_DELETED: &str = "user.deleted";
}

#[derive(Clone)]
pub struct AuditLogService {
    repo: AuditLogRepository,
}

impl AuditLogService {
    pub fn new(repo: AuditLogRepository) -> Self {
        Self { repo }
    }

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

    pub async fn list(
        &self,
        query: &ListAuditLogQuery,
    ) -> AppResult<PaginatedResponse<AuditLogEntryResponse>> {
        let pagination = query.pagination();
        let (entries, total) = self
            .repo
            .list(pagination.limit(), pagination.offset())
            .await?;
        Ok(PaginatedResponse::new(
            entries
                .into_iter()
                .map(AuditLogEntryResponse::from)
                .collect(),
            pagination,
            total,
        ))
    }
}
