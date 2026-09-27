use std::sync::Arc;

use axum::{Json, extract::State};

use crate::{
    common::{
        dto::PaginatedResponse, error::AppResult, extractors::ValidatedQuery, security::AdminUser,
    },
    modules::users::{
        dto::{ListUsersQuery, UserResponse},
        services::UsersService,
    },
};

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
