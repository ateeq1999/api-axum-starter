//! POST /auth/qr/sessions

use crate::common::{error::AppResult, extractors::ClientMeta};
use crate::modules::qr_login::{dto::CreatedSession, services::QrLoginService};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/qr/sessions",
    responses((status = 201, description = "New session: show the QR code and verification code on this device", body = CreatedSession)),
    tag = "qr-login"
)]
pub(crate) async fn create(
    State(qr): State<Arc<QrLoginService>>,
    meta: ClientMeta,
) -> AppResult<(StatusCode, Json<CreatedSession>)> {
    Ok((StatusCode::CREATED, Json(qr.create(meta).await?)))
}
