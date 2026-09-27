use axum::{Router, routing::get};

use crate::state::AppState;

pub mod delete_media;
pub mod download_content;
pub mod get_metadata;
pub mod list_media;
pub mod upload_media;

use delete_media::remove;
use download_content::content;
use get_metadata::metadata;
use list_media::list;
use upload_media::upload;

/// Upload, list, fetch and delete the caller's own files (admins may also fetch and delete any).
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(upload))
        .route("/{id}", get(metadata).delete(remove))
        .route("/{id}/content", get(content))
}
