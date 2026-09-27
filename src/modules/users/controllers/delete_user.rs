use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    common::{error::AppResult, security::AdminUser},
    modules::{avatars::AvatarService, media::MediaService, users::services::UsersService},
};

#[utoipa::path(
    delete,
    path = "/api/v1/users/{id}",
    params(("id" = Uuid, Path, description = "User id")),
    responses(
        (status = 204, description = "User soft-deleted"),
        (status = 400, description = "Cannot delete self, or would remove the last admin"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn remove(
    AdminUser(actor): AdminUser,
    State(users): State<Arc<UsersService>>,
    State(avatars): State<Arc<AvatarService>>,
    State(media): State<Arc<MediaService>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    if let Some(avatar_key) = users.delete(&actor, id).await? {
        avatars.remove_file(&avatar_key).await;
    }
    // The account is already gone, so a cleanup failure must not turn this into an error: the
    // orphaned files are unreachable (their rows are deleted with the owner).
    if let Err(error) = media.delete_all_for_user(id).await {
        tracing::warn!(error = ?error, user_id = %id, "could not clean up a deleted user's media");
    }
    Ok(StatusCode::NO_CONTENT)
}
