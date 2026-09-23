use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
};
use uuid::Uuid;

use super::{
    dto::{ApiKeyResponse, CreateApiKeyDto, CreatedApiKeyResponse},
    service::ApiKeysService,
};
use crate::{
    common::{error::AppResult, extractors::ValidatedJson, security::SessionUser},
    state::AppState,
};

/// API keys are managed with an interactive sign-in only: a key cannot mint or revoke keys.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", delete(revoke))
}

#[utoipa::path(
    post,
    path = "/api/v1/api-keys",
    request_body = CreateApiKeyDto,
    responses((status = 201, description = "Key created; the raw key is shown only in this response", body = CreatedApiKeyResponse)),
    security(("bearer_auth" = [])),
    tag = "api-keys"
)]
pub(crate) async fn create(
    actor: SessionUser,
    State(keys): State<Arc<ApiKeysService>>,
    ValidatedJson(dto): ValidatedJson<CreateApiKeyDto>,
) -> AppResult<(StatusCode, Json<CreatedApiKeyResponse>)> {
    Ok((StatusCode::CREATED, Json(keys.create(&actor, dto).await?)))
}

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
