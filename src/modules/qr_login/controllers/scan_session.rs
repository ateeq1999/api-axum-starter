//! POST /auth/qr/sessions/{id}/scan

use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::qr_login::{dto::ScanResponse, services::QrLoginService};
use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/qr/sessions/{id}/scan",
    params(("id" = String, Path, description = "Session id")),
    responses(
        (status = 200, description = "Requester details to show the approving user", body = ScanResponse),
        (status = 400, description = "Already scanned by someone else"),
    ),
    security(("bearer_auth" = [])),
    tag = "qr-login"
)]
pub(crate) async fn scan(
    actor: SessionUser,
    State(qr): State<Arc<QrLoginService>>,
    Path(id): Path<String>,
) -> AppResult<Json<ScanResponse>> {
    Ok(Json(qr.scan(&actor, &id).await?))
}
