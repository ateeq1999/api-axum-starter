//! DELETE /media/{id}

use crate::common::{error::AppResult, security::AuthUser};
use crate::modules::media::services::MediaService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

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
