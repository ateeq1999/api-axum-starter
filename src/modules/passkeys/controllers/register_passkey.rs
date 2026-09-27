//! POST /auth/passkeys/register/begin and /register/finish

use crate::common::{error::AppResult, extractors::ValidatedJson, security::SessionUser};
use crate::modules::passkeys::{
    dto::{BeginResponse, FinishRegistrationDto, PasskeyResponse},
    services::PasskeysService,
};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;
use webauthn_rs::prelude::CreationChallengeResponse;

#[utoipa::path(
    post,
    path = "/api/v1/auth/passkeys/register/begin",
    responses((status = 200, description = "WebAuthn creation options plus a challenge_id; pass options to navigator.credentials.create()", body = Object)),
    security(("bearer_auth" = [])),
    tag = "passkeys"
)]
pub(crate) async fn begin_registration(
    actor: SessionUser,
    State(passkeys): State<Arc<PasskeysService>>,
) -> AppResult<Json<BeginResponse<CreationChallengeResponse>>> {
    Ok(Json(passkeys.begin_registration(&actor).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/passkeys/register/finish",
    request_body(content = Object, description = "{ challenge_id, name?, credential: <credential.toJSON()> }"),
    responses(
        (status = 201, description = "Passkey registered", body = PasskeyResponse),
        (status = 400, description = "Challenge invalid/expired, or the ceremony was rejected"),
    ),
    security(("bearer_auth" = [])),
    tag = "passkeys"
)]
pub(crate) async fn finish_registration(
    actor: SessionUser,
    State(passkeys): State<Arc<PasskeysService>>,
    ValidatedJson(dto): ValidatedJson<FinishRegistrationDto>,
) -> AppResult<(StatusCode, Json<PasskeyResponse>)> {
    Ok((
        StatusCode::CREATED,
        Json(passkeys.finish_registration(&actor, dto).await?),
    ))
}
