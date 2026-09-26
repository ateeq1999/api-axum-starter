use std::sync::Arc;

use axum::{
    Json, Router,
    body::to_bytes,
    extract::{Path, Request, State},
    http::{
        HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
    routing::get,
};
use uuid::Uuid;

use super::{
    dto::{ListMediaQuery, MediaResponse, UploadMediaQuery},
    error::MediaError,
    service::MediaService,
};
use crate::{
    common::{
        dto::PaginatedResponse, error::AppResult, extractors::ValidatedQuery, security::AuthUser,
    },
    state::AppState,
};

/// Upload, list, fetch and delete the caller's own files (admins may also fetch and delete any).
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(upload))
        .route("/{id}", get(metadata).delete(remove))
        .route("/{id}/content", get(content))
}

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

#[utoipa::path(
    get,
    path = "/api/v1/media",
    params(ListMediaQuery),
    responses((status = 200, description = "The caller's files, newest first", body = PaginatedResponse<MediaResponse>)),
    security(("bearer_auth" = [])),
    tag = "media"
)]
pub(crate) async fn list(
    actor: AuthUser,
    State(media): State<Arc<MediaService>>,
    ValidatedQuery(query): ValidatedQuery<ListMediaQuery>,
) -> AppResult<Json<PaginatedResponse<MediaResponse>>> {
    Ok(Json(media.list(&actor, query.pagination()).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/media/{id}",
    params(("id" = Uuid, Path, description = "Media id")),
    responses(
        (status = 200, description = "File metadata", body = MediaResponse),
        (status = 404, description = "No such file, or it belongs to someone else"),
    ),
    security(("bearer_auth" = [])),
    tag = "media"
)]
pub(crate) async fn metadata(
    actor: AuthUser,
    State(media): State<Arc<MediaService>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<MediaResponse>> {
    Ok(Json(media.metadata(&actor, id).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/media/{id}/content",
    params(("id" = Uuid, Path, description = "Media id")),
    responses(
        (status = 200, description = "The file's bytes, with the detected Content-Type"),
        (status = 404, description = "No such file, or it belongs to someone else"),
    ),
    security(("bearer_auth" = [])),
    tag = "media"
)]
pub(crate) async fn content(
    actor: AuthUser,
    State(media): State<Arc<MediaService>>,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    let stored = media.content(&actor, id).await?;

    let mut response = (StatusCode::OK, stored.bytes).into_response();
    let headers = response.headers_mut();
    if let Ok(content_type) = HeaderValue::from_str(&stored.media.content_type) {
        headers.insert(CONTENT_TYPE, content_type);
    }
    // Uploads are untrusted content served from the API's own origin. Only passive media is
    // rendered inline; everything else downloads. `nosniff` stops the browser second-guessing
    // the type, and the CSP sandbox keeps any document that does get rendered inert.
    let disposition = content_disposition(
        &stored.media.content_type,
        stored.media.original_filename.as_deref(),
    );
    if let Ok(disposition) = HeaderValue::from_str(&disposition) {
        headers.insert(CONTENT_DISPOSITION, disposition);
    }
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=3600"),
    );
    Ok(response)
}

#[utoipa::path(
    delete,
    path = "/api/v1/media/{id}",
    params(("id" = Uuid, Path, description = "Media id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "No such file, or it belongs to someone else"),
    ),
    security(("bearer_auth" = [])),
    tag = "media"
)]
pub(crate) async fn remove(
    actor: AuthUser,
    State(media): State<Arc<MediaService>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    media.delete(&actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn content_disposition(content_type: &str, original_filename: Option<&str>) -> String {
    let passive = ["image/", "video/", "audio/"]
        .iter()
        .any(|prefix| content_type.starts_with(prefix));
    let kind = if passive { "inline" } else { "attachment" };
    format!(
        "{kind}; filename=\"{}\"",
        safe_download_name(original_filename)
    )
}

/// Reduces a client-supplied name to characters that are safe inside a quoted header value.
fn safe_download_name(original_filename: Option<&str>) -> String {
    let cleaned: String = original_filename
        .unwrap_or("")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.trim_matches(['.', '_']).is_empty() {
        "download".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_names_are_reduced_to_safe_characters() {
        assert_eq!(
            safe_download_name(Some("holiday photo.png")),
            "holiday_photo.png"
        );
        assert_eq!(safe_download_name(Some("a\"b\r\nc.pdf")), "a_b__c.pdf");
        assert_eq!(
            safe_download_name(Some("../../etc/passwd")),
            ".._.._etc_passwd"
        );
        assert_eq!(safe_download_name(Some("...")), "download");
        assert_eq!(safe_download_name(None), "download");
    }

    #[test]
    fn only_passive_media_is_shown_inline() {
        assert!(content_disposition("image/png", Some("a.png")).starts_with("inline;"));
        assert!(content_disposition("audio/mpeg", None).starts_with("inline;"));
        assert!(content_disposition("application/pdf", Some("a.pdf")).starts_with("attachment;"));
    }
}
