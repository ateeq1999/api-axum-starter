use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    routing::{delete, get, post},
};
use uuid::Uuid;
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse};

use super::{
    dto::{BeginResponse, FinishLoginDto, FinishRegistrationDto, PasskeyResponse},
    service::PasskeysService,
};
use crate::{
    common::{
        error::AppResult,
        extractors::ValidatedJson,
        middleware::rate_limit::{self, RateLimiter},
        security::SessionUser,
    },
    modules::auth::dto::TokenResponse,
    state::AppState,
};

/// Mounted at `/api/v1/auth/passkeys`.
///
/// Managing passkeys needs an interactive sign-in (not an API key). Signing in with one is public
/// and rate limited.
pub fn router(limiter: RateLimiter) -> Router<AppState> {
    let public = Router::new()
        .route("/login/begin", post(begin_login))
        .route("/login/finish", post(finish_login))
        .route_layer(middleware::from_fn_with_state(limiter, rate_limit::enforce));

    public
        .route("/", get(list))
        .route("/register/begin", post(begin_registration))
        .route("/register/finish", post(finish_registration))
        .route("/{id}", delete(remove))
}

// WebAuthn ceremony payloads (`webauthn-rs` types) are opaque, browser-generated JSON blobs not
// meant for manual construction, so they are documented as plain objects here rather than fully
// modeled field-by-field.

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
