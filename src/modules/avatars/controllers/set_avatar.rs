//! PUT /users/me/avatar

use crate::modules::avatars::{
    error::AvatarError,
    services::{AvatarService, MAX_UPLOAD_BYTES},
};
use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::users::dto::UserResponse,
};
use axum::{
    Json,
    body::to_bytes,
    extract::{Request, State},
    http::header::CONTENT_LENGTH,
};
use std::sync::Arc;

#[utoipa::path(
    put,
    path = "/api/v1/users/me/avatar",
    request_body(content_type = "application/octet-stream", description = "Raw image bytes (JPEG/PNG/WebP/GIF), 2 MiB max"),
    responses(
        (status = 200, description = "Updated profile, including the new avatar_url", body = UserResponse),
        (status = 413, description = "Upload larger than 2 MiB"),
        (status = 422, description = "Not a decodable image"),
    ),
    security(("bearer_auth" = [])),
    tag = "avatars"
)]
pub(crate) async fn upload(
    actor: AuthUser,
    State(avatars): State<Arc<AvatarService>>,
    request: Request,
) -> AppResult<Json<UserResponse>> {
    let too_large = || AvatarError::TooLarge(MAX_UPLOAD_BYTES / (1024 * 1024));

    let declared = request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|len| len > MAX_UPLOAD_BYTES) {
        return Err(too_large().into());
    }
    let body = to_bytes(request.into_body(), MAX_UPLOAD_BYTES)
        .await
        .map_err(|_| too_large())?;

    Ok(Json(avatars.set(&actor, body).await?))
}
