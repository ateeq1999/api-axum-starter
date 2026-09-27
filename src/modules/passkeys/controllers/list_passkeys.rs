//! GET /auth/passkeys

use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::passkeys::{dto::PasskeyResponse, services::PasskeysService};
use axum::{Json, extract::State};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/v1/auth/passkeys",
    responses((status = 200, description = "The caller's own passkeys", body = [PasskeyResponse])),
    security(("bearer_auth" = [])),
    tag = "passkeys"
)]
pub(crate) async fn list(
    actor: SessionUser,
    State(passkeys): State<Arc<PasskeysService>>,
) -> AppResult<Json<Vec<PasskeyResponse>>> {
    Ok(Json(passkeys.list(&actor).await?))
}
