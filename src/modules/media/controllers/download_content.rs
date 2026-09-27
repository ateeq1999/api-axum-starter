//! GET /media/{id}/content

use crate::common::{error::AppResult, security::AuthUser};
use crate::modules::media::services::MediaService;
use axum::{
    extract::{Path, State},
    http::{
        HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;

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
