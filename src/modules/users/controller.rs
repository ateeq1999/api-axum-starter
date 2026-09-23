use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use uuid::Uuid;

use super::{
    dto::{CreateUserDto, ListUsersQuery, UpdateProfileDto, UpdateUserDto, UserResponse},
    service::UsersService,
};
use crate::{
    common::{
        dto::PaginatedResponse,
        error::AppResult,
        extractors::{ValidatedJson, ValidatedQuery},
        security::{AdminUser, AuthUser},
    },
    modules::avatars::AvatarService,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/me", get(me).patch(update_me))
        .route("/{id}", get(get_one).patch(update).delete(remove))
}

#[utoipa::path(
    get,
    path = "/api/v1/users",
    params(ListUsersQuery),
    responses((status = 200, description = "Paginated user list", body = PaginatedResponse<UserResponse>)),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn list(
    _admin: AdminUser,
    State(users): State<Arc<UsersService>>,
    ValidatedQuery(query): ValidatedQuery<ListUsersQuery>,
) -> AppResult<Json<PaginatedResponse<UserResponse>>> {
    Ok(Json(users.list(&query).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/users",
    request_body = CreateUserDto,
    responses(
        (status = 201, description = "User created", body = UserResponse),
        (status = 409, description = "Email already registered"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn create(
    AdminUser(actor): AdminUser,
    State(users): State<Arc<UsersService>>,
    ValidatedJson(dto): ValidatedJson<CreateUserDto>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    Ok((StatusCode::CREATED, Json(users.create(&actor, dto).await?)))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    responses((status = 200, description = "The signed-in user", body = UserResponse)),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn me(
    actor: AuthUser,
    State(users): State<Arc<UsersService>>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.get(&actor, actor.id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/v1/users/me",
    request_body = UpdateProfileDto,
    responses((status = 200, description = "Updated profile", body = UserResponse)),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn update_me(
    actor: AuthUser,
    State(users): State<Arc<UsersService>>,
    ValidatedJson(dto): ValidatedJson<UpdateProfileDto>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.update_profile(&actor, dto).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/{id}",
    params(("id" = Uuid, Path, description = "User id")),
    responses(
        (status = 200, description = "The requested user", body = UserResponse),
        (status = 403, description = "Not allowed to view this user"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn get_one(
    actor: AuthUser,
    State(users): State<Arc<UsersService>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.get(&actor, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/v1/users/{id}",
    params(("id" = Uuid, Path, description = "User id")),
    request_body = UpdateUserDto,
    responses(
        (status = 200, description = "Updated user", body = UserResponse),
        (status = 400, description = "Would lock the actor out or remove the last admin"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn update(
    AdminUser(actor): AdminUser,
    State(users): State<Arc<UsersService>>,
    Path(id): Path<Uuid>,
    ValidatedJson(dto): ValidatedJson<UpdateUserDto>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.update(&actor, id, dto).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/users/{id}",
    params(("id" = Uuid, Path, description = "User id")),
    responses(
        (status = 204, description = "User soft-deleted"),
        (status = 400, description = "Cannot delete self, or would remove the last admin"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "users"
)]
pub(crate) async fn remove(
    AdminUser(actor): AdminUser,
    State(users): State<Arc<UsersService>>,
    State(avatars): State<Arc<AvatarService>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    if let Some(avatar_key) = users.delete(&actor, id).await? {
        avatars.remove_file(&avatar_key).await;
    }
    Ok(StatusCode::NO_CONTENT)
}
