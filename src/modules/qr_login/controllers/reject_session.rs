//! POST /auth/qr/sessions/{id}/reject

use crate::common::{dto::MessageResponse, error::AppResult, security::SessionUser};
use crate::modules::qr_login::services::QrLoginService;
use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/qr/sessions/{id}/reject",
    params(("id" = String, Path, description = "Session id")),
    responses((status = 200, description = "Rejected", body = MessageResponse)),
    security(("bearer_auth" = [])),
    tag = "qr-login"
)]
pub(crate) async fn reject(
    actor: SessionUser,
    State(qr): State<Arc<QrLoginService>>,
    Path(id): Path<String>,
) -> AppResult<Json<MessageResponse>> {
    qr.reject(&actor, &id).await?;
    Ok(Json(MessageResponse::new("Login rejected.")))
}
