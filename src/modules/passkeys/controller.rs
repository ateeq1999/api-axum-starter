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

async fn begin_registration(
    actor: SessionUser,
    State(passkeys): State<Arc<PasskeysService>>,
) -> AppResult<Json<BeginResponse<CreationChallengeResponse>>> {
    Ok(Json(passkeys.begin_registration(&actor).await?))
}

async fn finish_registration(
    actor: SessionUser,
    State(passkeys): State<Arc<PasskeysService>>,
    ValidatedJson(dto): ValidatedJson<FinishRegistrationDto>,
) -> AppResult<(StatusCode, Json<PasskeyResponse>)> {
    Ok((
        StatusCode::CREATED,
        Json(passkeys.finish_registration(&actor, dto).await?),
    ))
}

async fn begin_login(
    State(passkeys): State<Arc<PasskeysService>>,
) -> AppResult<Json<BeginResponse<RequestChallengeResponse>>> {
    Ok(Json(passkeys.begin_login().await?))
}

async fn finish_login(
    State(passkeys): State<Arc<PasskeysService>>,
    ValidatedJson(dto): ValidatedJson<FinishLoginDto>,
) -> AppResult<Json<TokenResponse>> {
    Ok(Json(passkeys.finish_login(dto).await?))
}

async fn list(
    actor: SessionUser,
    State(passkeys): State<Arc<PasskeysService>>,
) -> AppResult<Json<Vec<PasskeyResponse>>> {
    Ok(Json(passkeys.list(&actor).await?))
}

async fn remove(
    actor: SessionUser,
    State(passkeys): State<Arc<PasskeysService>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    passkeys.delete(&actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
