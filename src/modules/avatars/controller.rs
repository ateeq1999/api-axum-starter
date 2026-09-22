use std::sync::Arc;

use axum::{
    Json, Router,
    body::to_bytes,
    extract::{Path, Request, State},
    http::{
        HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
    routing::{get, put},
};

use super::{
    error::AvatarError,
    service::{AvatarService, MAX_UPLOAD_BYTES},
};
use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::users::dto::UserResponse,
    state::AppState,
};

/// `PUT /users/me/avatar` (raw image bytes as the body) and `DELETE /users/me/avatar`.
pub fn own_router() -> Router<AppState> {
    Router::new().route("/", put(upload).delete(remove))
}

/// `GET /avatars/{file}`: public, so `<img src>` works without an Authorization header.
/// The file name is an unguessable random id that changes on every upload.
pub fn public_router() -> Router<AppState> {
    Router::new().route("/{file}", get(serve))
}

async fn upload(
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

async fn remove(
    actor: AuthUser,
    State(avatars): State<Arc<AvatarService>>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(avatars.remove(&actor).await?))
}

async fn serve(
    State(avatars): State<Arc<AvatarService>>,
    Path(file): Path<String>,
) -> AppResult<Response> {
    let bytes = avatars.read(&file).await?;
    let mut response = (StatusCode::OK, bytes).into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    // The name changes on every upload, so a given URL never changes: cache it for a year.
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("cross-origin"),
    );
    Ok(response)
}
