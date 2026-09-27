//! The admin-readable log, newest first.

use super::AuditLogService;
use crate::common::{dto::PaginatedResponse, error::AppResult};
use crate::modules::audit_log::dto::{AuditLogEntryResponse, ListAuditLogQuery};

impl AuditLogService {
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
