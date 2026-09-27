//! GET /avatars/{file}

use crate::common::error::AppResult;
use crate::modules::avatars::services::AvatarService;
use axum::{
    extract::{Path, State},
    http::{
        HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/v1/avatars/{file}",
    params(("file" = String, Path, description = "Generated avatar file name")),
    responses(
        (status = 200, description = "The JPEG image", content_type = "image/jpeg"),
        (status = 404, description = "No such file"),
    ),
    tag = "avatars"
)]
pub(crate) async fn serve(
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
