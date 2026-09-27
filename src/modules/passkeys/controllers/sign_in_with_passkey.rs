//! POST /auth/passkeys/login/begin and /login/finish

use crate::modules::passkeys::{
    dto::{BeginResponse, FinishLoginDto},
    services::PasskeysService,
};
use crate::{
    common::{error::AppResult, extractors::ValidatedJson},
    modules::auth::dto::TokenResponse,
};
use axum::{Json, extract::State};
use std::sync::Arc;
use webauthn_rs::prelude::RequestChallengeResponse;

#[utoipa::path(
    post,
    path = "/api/v1/auth/passkeys/login/begin",
    responses((status = 200, description = "Usernameless WebAuthn request options plus a challenge_id", body = Object)),
    tag = "passkeys"
)]
pub(crate) async fn begin_login(
    State(passkeys): State<Arc<PasskeysService>>,
) -> AppResult<Json<BeginResponse<RequestChallengeResponse>>> {
    Ok(Json(passkeys.begin_login().await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/passkeys/login/finish",
    request_body(content = Object, description = "{ challenge_id, credential: <credential.toJSON()> }"),
    responses(
        (status = 200, description = "Signed in", body = TokenResponse),
        (status = 400, description = "Challenge invalid/expired, or the ceremony was rejected"),
    ),
    tag = "passkeys"
)]
pub(crate) async fn finish_login(
    State(passkeys): State<Arc<PasskeysService>>,
    ValidatedJson(dto): ValidatedJson<FinishLoginDto>,
) -> AppResult<Json<TokenResponse>> {
    Ok(Json(passkeys.finish_login(dto).await?))
}
