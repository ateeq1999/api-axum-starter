use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    middleware,
    routing::{get, post},
};

use super::{
    dto::{ApproveDto, CreatedSession, PollResponse, ScanResponse},
    service::QrLoginService,
};
use crate::{
    common::{
        dto::MessageResponse,
        error::AppResult,
        extractors::{ClientMeta, ValidatedJson},
        middleware::rate_limit::{self, RateLimiter},
        security::SessionUser,
    },
    state::AppState,
};

const SECRET_HEADER: &str = "x-qr-secret";

/// Mounted at `/api/v1/auth/qr`.
///
/// * new device: `POST /sessions` (public), then poll `GET /sessions/{id}` with `X-QR-Secret`
/// * signed-in device: `POST /sessions/{id}/scan`, then `/approve` or `/reject` (interactive
///   sign-in required: an API key cannot approve a login)
pub fn router(create_limiter: RateLimiter, poll_limiter: RateLimiter) -> Router<AppState> {
    let create =
        Router::new()
            .route("/sessions", post(create))
            .route_layer(middleware::from_fn_with_state(
                create_limiter,
                rate_limit::enforce,
            ));
    let poll = Router::new()
        .route("/sessions/{id}", get(poll))
        .route_layer(middleware::from_fn_with_state(
            poll_limiter,
            rate_limit::enforce,
        ));

    create
        .merge(poll)
        .route("/sessions/{id}/scan", post(scan))
        .route("/sessions/{id}/approve", post(approve))
        .route("/sessions/{id}/reject", post(reject))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/qr/sessions",
    responses((status = 201, description = "New session: show the QR code and verification code on this device", body = CreatedSession)),
    tag = "qr-login"
)]
pub(crate) async fn create(
    State(qr): State<Arc<QrLoginService>>,
    meta: ClientMeta,
) -> AppResult<(StatusCode, Json<CreatedSession>)> {
    Ok((StatusCode::CREATED, Json(qr.create(meta).await?)))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/qr/sessions/{id}",
    params(("id" = String, Path, description = "Session id")),
    responses((status = 200, description = "Current status; carries the access token exactly once, right after approval", body = PollResponse)),
    tag = "qr-login"
)]
pub(crate) async fn poll(
    State(qr): State<Arc<QrLoginService>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Json<PollResponse>> {
    let secret = headers.get(SECRET_HEADER).and_then(|v| v.to_str().ok());
    Ok(Json(qr.poll(&id, secret).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/qr/sessions/{id}/scan",
    params(("id" = String, Path, description = "Session id")),
    responses(
        (status = 200, description = "Requester details to show the approving user", body = ScanResponse),
        (status = 400, description = "Already scanned by someone else"),
    ),
    security(("bearer_auth" = [])),
    tag = "qr-login"
)]
pub(crate) async fn scan(
    actor: SessionUser,
    State(qr): State<Arc<QrLoginService>>,
    Path(id): Path<String>,
) -> AppResult<Json<ScanResponse>> {
    Ok(Json(qr.scan(&actor, &id).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/qr/sessions/{id}/approve",
    params(("id" = String, Path, description = "Session id")),
    request_body = ApproveDto,
    responses(
        (status = 200, description = "Approved", body = MessageResponse),
        (status = 400, description = "Wrong verification code, or too many attempts"),
    ),
    security(("bearer_auth" = [])),
    tag = "qr-login"
)]
pub(crate) async fn approve(
    actor: SessionUser,
    State(qr): State<Arc<QrLoginService>>,
    Path(id): Path<String>,
    ValidatedJson(dto): ValidatedJson<ApproveDto>,
) -> AppResult<Json<MessageResponse>> {
    qr.approve(&actor, &id, dto).await?;
    Ok(Json(MessageResponse::new("Login approved.")))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/qr/sessions/{id}/reject",
    params(("id" = String, Path, description = "Session id")),
    responses((status = 200, description = "Rejected", body = MessageResponse)),
    security(("bearer_auth" = [])),
    tag = "qr-login"
)]
pub(crate) async fn reject(
    actor: SessionUser,
    State(qr): State<Arc<QrLoginService>>,
    Path(id): Path<String>,
) -> AppResult<Json<MessageResponse>> {
    qr.reject(&actor, &id).await?;
    Ok(Json(MessageResponse::new("Login rejected.")))
}
