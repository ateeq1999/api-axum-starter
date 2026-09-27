//! DELETE /auth/passkeys/{id}

use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::passkeys::services::PasskeysService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

#[utoipa::path(
    delete,
    path = "/api/v1/auth/passkeys/{id}",
    params(("id" = Uuid, Path, description = "Passkey id")),
    responses(
        (status = 204, description = "Removed"),
        (status = 400, description = "The account's last sign-in method"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "passkeys"
)]
pub(crate) async fn remove(
    actor: SessionUser,
    State(passkeys): State<Arc<PasskeysService>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    passkeys.delete(&actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
