use std::sync::Arc;

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};

use crate::{
    common::{
        dto::MessageResponse,
        error::AppResult,
        extractors::ValidatedJson,
        security::{AdminUser, AuthUser},
    },
    modules::{
        auth::{
            dto::{ChangePasswordDto, ForgotPasswordDto, InviteUserDto, ResetPasswordDto},
            services::AuthService,
        },
        users::dto::UserResponse,
    },
    state::AppState,
};

pub fn rate_limited() -> Router<AppState> {
    Router::new()
        .route("/password/forgot", post(forgot))
        .route("/password/reset", post(reset))
}

pub fn authenticated() -> Router<AppState> {
    Router::new()
        .route("/password/change", post(change))
        .route("/invitations", post(invite))
}

async fn forgot(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<ForgotPasswordDto>,
) -> AppResult<(StatusCode, Json<MessageResponse>)> {
    auth.password_reset.forgot(dto).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(MessageResponse::new(
            "If that address is registered, we sent a link to reset the password.",
        )),
    ))
}

async fn reset(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<ResetPasswordDto>,
) -> AppResult<Json<MessageResponse>> {
    auth.password_reset.reset(dto).await?;
    Ok(Json(MessageResponse::new("Password updated.")))
}

async fn change(
    actor: AuthUser,
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<ChangePasswordDto>,
) -> AppResult<Json<MessageResponse>> {
    auth.password_reset.change(&actor, dto).await?;
    Ok(Json(MessageResponse::new("Password updated.")))
}

async fn invite(
    _admin: AdminUser,
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<InviteUserDto>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    Ok((
        StatusCode::CREATED,
        Json(auth.password_reset.invite(dto).await?),
    ))
}
