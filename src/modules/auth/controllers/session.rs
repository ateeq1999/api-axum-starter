use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};

use crate::{
    common::{error::AppResult, extractors::ValidatedJson, security::AuthUser},
    modules::{
        auth::{
            dto::{CredentialsDto, LoginResponse},
            services::AuthService,
        },
        users::dto::UserResponse,
    },
    state::AppState,
};

/// Public routes that are worth rate limiting.
pub fn rate_limited() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
}

pub fn authenticated() -> Router<AppState> {
    Router::new().route("/me", get(me))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = CredentialsDto,
    responses(
        (status = 201, description = "Account created; a verification email was sent", body = UserResponse),
        (status = 409, description = "Email already registered"),
        (status = 400, description = "Password is too weak or has appeared in a known breach"),
    ),
    tag = "auth"
)]
pub(crate) async fn register(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<CredentialsDto>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    Ok((StatusCode::CREATED, Json(auth.session.register(dto).await?)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = CredentialsDto,
    responses(
        (status = 200, description = "Either a session (access_token) or, for a two-factor-enabled account, requires_totp + pending_token", body = LoginResponse),
        (status = 401, description = "Invalid email/password, or the account is temporarily locked"),
        (status = 403, description = "Account disabled or email not verified"),
    ),
    tag = "auth"
)]
pub(crate) async fn login(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<CredentialsDto>,
) -> AppResult<Json<LoginResponse>> {
    Ok(Json(auth.session.login(dto).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses((status = 200, description = "The signed-in user", body = UserResponse)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn me(
    actor: AuthUser,
    State(auth): State<Arc<AuthService>>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(auth.session.me(&actor).await?))
}
