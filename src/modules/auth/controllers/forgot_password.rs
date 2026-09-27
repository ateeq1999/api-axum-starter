//! POST /auth/password/forgot

use crate::{
    common::{dto::MessageResponse, error::AppResult, extractors::ValidatedJson},
    modules::auth::{dto::ForgotPasswordDto, services::AuthService},
};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/auth/password/forgot",
    request_body = ForgotPasswordDto,
    responses((status = 202, description = "Always accepted, whether or not the address is registered", body = MessageResponse)),
    tag = "auth"
)]
pub(crate) async fn forgot(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<ForgotPasswordDto>,
) -> AppResult<(StatusCode, Json<MessageResponse>)> {
    auth.password_reset.forgot(dto).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(MessageResponse::new(
            "If that address is registered, we sent a link to reset the password.",
        )),
    ))
}
