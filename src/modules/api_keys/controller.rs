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

async fn create(
    actor: SessionUser,
    State(keys): State<Arc<ApiKeysService>>,
    ValidatedJson(dto): ValidatedJson<CreateApiKeyDto>,
) -> AppResult<(StatusCode, Json<CreatedApiKeyResponse>)> {
    Ok((StatusCode::CREATED, Json(keys.create(&actor, dto).await?)))
}

async fn list(
    actor: SessionUser,
    State(keys): State<Arc<ApiKeysService>>,
) -> AppResult<Json<Vec<ApiKeyResponse>>> {
    Ok(Json(keys.list(&actor).await?))
}

async fn revoke(
    actor: SessionUser,
    State(keys): State<Arc<ApiKeysService>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    keys.revoke(&actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
