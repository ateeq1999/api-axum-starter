//! GET /media

use crate::common::{
    dto::PaginatedResponse, error::AppResult, extractors::ValidatedQuery, security::AuthUser,
};
use crate::modules::media::{
    dto::{ListMediaQuery, MediaResponse},
    services::MediaService,
};
use axum::{Json, extract::State};
use std::sync::Arc;

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
