use super::repository::AuditLogRepository;

mod list_entries;
mod record_entry;

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
}
