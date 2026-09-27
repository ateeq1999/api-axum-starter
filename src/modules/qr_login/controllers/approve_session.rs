//! POST /auth/qr/sessions/{id}/approve

use crate::common::{
    dto::MessageResponse, error::AppResult, extractors::ValidatedJson, security::SessionUser,
};
use crate::modules::qr_login::{dto::ApproveDto, services::QrLoginService};
use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/qr/sessions/{id}/approve",
    params(("id" = String, Path, description = "Session id")),
    request_body = ApproveDto,
    responses(
        (status = 200, description = "Approved", body = MessageResponse),
        (status = 400, description = "Wrong verification code, or too many attempts"),
    ),
    security(("bearer_auth" = [])),
    tag = "qr-login"
)]
pub(crate) async fn approve(
    actor: SessionUser,
    State(qr): State<Arc<QrLoginService>>,
    Path(id): Path<String>,
    ValidatedJson(dto): ValidatedJson<ApproveDto>,
) -> AppResult<Json<MessageResponse>> {
    qr.approve(&actor, &id, dto).await?;
    Ok(Json(MessageResponse::new("Login approved.")))
}
