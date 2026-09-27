//! GET /media/{id}

use crate::common::{error::AppResult, security::AuthUser};
use crate::modules::media::{dto::MediaResponse, services::MediaService};
use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;
use uuid::Uuid;

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
