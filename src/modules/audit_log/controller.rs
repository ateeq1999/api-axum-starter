use std::sync::Arc;

use axum::{Json, Router, extract::State, routing::get};

use super::{
    dto::{AuditLogEntryResponse, ListAuditLogQuery},
    service::AuditLogService,
};
use crate::{
    common::{
        dto::PaginatedResponse, error::AppResult, extractors::ValidatedQuery, security::AdminUser,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list))
}

#[utoipa::path(
    get,
    path = "/api/v1/audit-log",
    params(ListAuditLogQuery),
    responses((status = 200, description = "Paginated administrative action log, newest first", body = PaginatedResponse<AuditLogEntryResponse>)),
    security(("bearer_auth" = [])),
    tag = "audit-log"
)]
pub(crate) async fn list(
    _admin: AdminUser,
    State(audit_log): State<Arc<AuditLogService>>,
    ValidatedQuery(query): ValidatedQuery<ListAuditLogQuery>,
) -> AppResult<Json<PaginatedResponse<AuditLogEntryResponse>>> {
    Ok(Json(audit_log.list(&query).await?))
}
