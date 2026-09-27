//! DELETE /api-keys/{id}

use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::api_keys::services::ApiKeysService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

#[utoipa::path(
    delete,
    path = "/api/v1/api-keys/{id}",
    params(("id" = Uuid, Path, description = "API key id")),
    responses(
        (status = 204, description = "Key revoked"),
        (status = 404, description = "Key not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "api-keys"
)]
pub(crate) async fn revoke(
    actor: SessionUser,
    State(keys): State<Arc<ApiKeysService>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    keys.revoke(&actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
