//! POST /api-keys

use crate::common::{error::AppResult, extractors::ValidatedJson, security::SessionUser};
use crate::modules::api_keys::{
    dto::{CreateApiKeyDto, CreatedApiKeyResponse},
    services::ApiKeysService,
};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

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
