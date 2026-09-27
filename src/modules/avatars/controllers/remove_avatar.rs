//! DELETE /users/me/avatar

use crate::modules::avatars::services::AvatarService;
use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::users::dto::UserResponse,
};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    delete,
    path = "/api/v1/users/me/avatar",
    responses((status = 200, description = "Updated profile, avatar_url now null", body = UserResponse)),
    security(("bearer_auth" = [])),
    tag = "avatars"
)]
pub(crate) async fn remove(
    actor: AuthUser,
    State(avatars): State<Arc<AvatarService>>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(avatars.remove(&actor).await?))
}
