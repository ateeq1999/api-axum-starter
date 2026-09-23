use std::sync::Arc;

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};

use crate::{
    common::{
        dto::MessageResponse,
        error::AppResult,
        extractors::ValidatedJson,
        security::{AdminUser, SessionUser},
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

#[utoipa::path(
    post,
    path = "/api/v1/auth/password/forgot",
    request_body = ForgotPasswordDto,
    responses((status = 202, description = "Always accepted, whether or not the address is registered", body = MessageResponse)),
    tag = "auth"
)]
pub(crate) async fn forgot(
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

#[utoipa::path(
    post,
    path = "/api/v1/auth/password/reset",
    request_body = ResetPasswordDto,
    responses(
        (status = 200, description = "Password updated", body = MessageResponse),
        (status = 400, description = "Link invalid/expired, or the password is too weak"),
    ),
    tag = "auth"
)]
pub(crate) async fn reset(
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<ResetPasswordDto>,
) -> AppResult<Json<MessageResponse>> {
    auth.password_reset.reset(dto).await?;
    Ok(Json(MessageResponse::new("Password updated.")))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/password/change",
    request_body = ChangePasswordDto,
    responses(
        (status = 200, description = "Password updated", body = MessageResponse),
        (status = 400, description = "Wrong current password, or the new one is too weak"),
    ),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn change(
    SessionUser(actor): SessionUser,
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<ChangePasswordDto>,
) -> AppResult<Json<MessageResponse>> {
    auth.password_reset.change(&actor, dto).await?;
    Ok(Json(MessageResponse::new("Password updated.")))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/invitations",
    request_body = InviteUserDto,
    responses((status = 201, description = "Account created; a \"set your password\" link was emailed", body = UserResponse)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn invite(
    _admin: AdminUser,
    State(auth): State<Arc<AuthService>>,
    ValidatedJson(dto): ValidatedJson<InviteUserDto>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    Ok((
        StatusCode::CREATED,
        Json(auth.password_reset.invite(dto).await?),
    ))
}
