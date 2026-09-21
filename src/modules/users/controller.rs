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
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/me", get(me).patch(update_me))
        .route("/{id}", get(get_one).patch(update).delete(remove))
}

async fn list(
    _admin: AdminUser,
    State(users): State<Arc<UsersService>>,
    ValidatedQuery(query): ValidatedQuery<ListUsersQuery>,
) -> AppResult<Json<PaginatedResponse<UserResponse>>> {
    Ok(Json(users.list(&query).await?))
}

async fn create(
    _admin: AdminUser,
    State(users): State<Arc<UsersService>>,
    ValidatedJson(dto): ValidatedJson<CreateUserDto>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    Ok((StatusCode::CREATED, Json(users.create(dto).await?)))
}

async fn me(
    actor: AuthUser,
    State(users): State<Arc<UsersService>>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.get(&actor, actor.id).await?))
}

async fn update_me(
    actor: AuthUser,
    State(users): State<Arc<UsersService>>,
    ValidatedJson(dto): ValidatedJson<UpdateProfileDto>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.update_profile(&actor, dto).await?))
}

async fn get_one(
    actor: AuthUser,
    State(users): State<Arc<UsersService>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.get(&actor, id).await?))
}

async fn update(
    AdminUser(actor): AdminUser,
    State(users): State<Arc<UsersService>>,
    Path(id): Path<Uuid>,
    ValidatedJson(dto): ValidatedJson<UpdateUserDto>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(users.update(&actor, id, dto).await?))
}

async fn remove(
    AdminUser(actor): AdminUser,
    State(users): State<Arc<UsersService>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    users.delete(&actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
