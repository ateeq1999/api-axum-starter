//! GET /api-keys

use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::api_keys::{dto::ApiKeyResponse, services::ApiKeysService};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/v1/api-keys",
    responses((status = 200, description = "The caller's own active keys", body = [ApiKeyResponse])),
    security(("bearer_auth" = [])),
    tag = "api-keys"
)]
pub(crate) async fn list(
    actor: SessionUser,
    State(keys): State<Arc<ApiKeysService>>,
) -> AppResult<Json<Vec<ApiKeyResponse>>> {
    Ok(Json(keys.list(&actor).await?))
}
