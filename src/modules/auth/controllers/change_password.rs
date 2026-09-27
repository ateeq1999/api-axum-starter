//! POST /auth/password/change

use crate::{
    common::{
        dto::MessageResponse, error::AppResult, extractors::ValidatedJson, security::SessionUser,
    },
    modules::auth::{dto::ChangePasswordDto, services::AuthService},
};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/password/change",
    request_body = ChangePasswordDto,
    responses(
        (status = 200, description = "Password updated", body = MessageResponse),
        (status = 400, description = "Wrong current password, or the new one is too weak"),
    ),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn change(
    SessionUser(actor): SessionUser,
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<ChangePasswordDto>,
) -> AppResult<Json<MessageResponse>> {
    auth.password_reset.change(&actor, dto).await?;
    Ok(Json(MessageResponse::new("Password updated.")))
}
