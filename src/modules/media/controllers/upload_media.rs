//! POST /media

use crate::common::{error::AppResult, extractors::ValidatedQuery, security::AuthUser};
use crate::modules::media::{
    dto::{MediaResponse, UploadMediaQuery},
    error::MediaError,
    services::MediaService,
};
use axum::{
    Json,
    body::to_bytes,
    extract::{Request, State},
    http::{StatusCode, header::CONTENT_LENGTH},
};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/v1/media",
    params(UploadMediaQuery),
    request_body(content_type = "application/octet-stream", description = "The raw file bytes. The type is detected from the bytes; the request's Content-Type is ignored"),
    responses(
        (status = 201, description = "Stored", body = MediaResponse),
        (status = 413, description = "Larger than MEDIA_MAX_UPLOAD_BYTES"),
        (status = 415, description = "Unrecognized file type, or one that is not allowed"),
    ),
    security(("bearer_auth" = [])),
    tag = "media"
)]
pub(crate) async fn upload(
    actor: AuthUser,
    State(media): State<Arc<MediaService>>,
    ValidatedQuery(query): ValidatedQuery<UploadMediaQuery>,
    request: Request,
) -> AppResult<(StatusCode, Json<MediaResponse>)> {
    let limit = media.max_upload_bytes();
    let too_large = || MediaError::TooLarge(limit / (1024 * 1024));

    let declared = request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|len| len > limit) {
        return Err(too_large().into());
    }
    let body = to_bytes(request.into_body(), limit)
        .await
        .map_err(|_| too_large())?;

    let stored = media.upload(&actor, body, query.filename).await?;
    Ok((StatusCode::CREATED, Json(stored)))
}
