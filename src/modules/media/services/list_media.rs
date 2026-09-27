//! The caller's files, newest first.

use super::MediaService;
use crate::common::{
    dto::{PaginatedResponse, Pagination},
    error::AppResult,
    security::AuthUser,
};
use crate::modules::media::dto::MediaResponse;

impl MediaService {
    pub async fn list(
        &self,
        actor: &AuthUser,
        pagination: Pagination,
    ) -> AppResult<PaginatedResponse<MediaResponse>> {
        let (items, total) = self
            .repo
            .list_by_owner(actor.id, pagination.limit(), pagination.offset())
            .await?;
        Ok(PaginatedResponse::new(
            items.into_iter().map(MediaResponse::from).collect(),
            pagination,
            total,
        ))
    }
}
