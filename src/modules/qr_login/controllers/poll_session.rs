//! GET /auth/qr/sessions/{id}

use super::SECRET_HEADER;
use crate::common::error::AppResult;
use crate::modules::qr_login::{dto::PollResponse, services::QrLoginService};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/v1/auth/qr/sessions/{id}",
    params(("id" = String, Path, description = "Session id")),
    responses((status = 200, description = "Current status; carries the access token exactly once, right after approval", body = PollResponse)),
    tag = "qr-login"
)]
pub(crate) async fn poll(
    State(qr): State<Arc<QrLoginService>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Json<PollResponse>> {
    let secret = headers.get(SECRET_HEADER).and_then(|v| v.to_str().ok());
    Ok(Json(qr.poll(&id, secret).await?))
}
