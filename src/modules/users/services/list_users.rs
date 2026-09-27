//! The admin user list: search, sort, pagination.

use super::UsersService;
use crate::{
    common::{dto::PaginatedResponse, error::AppResult},
    modules::users::{
        dto::{ListUsersQuery, UserResponse},
        repositories::UserFilter,
    },
};

impl UsersService {
    pub async fn list(&self, query: &ListUsersQuery) -> AppResult<PaginatedResponse<UserResponse>> {
        let pagination = query.pagination();
        let search = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
        let (users, total) = self
            .repo
            .list(&UserFilter {
                search,
                sort: query.sort.unwrap_or_default(),
                order: query.order.unwrap_or_default(),
                limit: pagination.limit(),
                offset: pagination.offset(),
            })
            .await?;
        Ok(PaginatedResponse::new(
            users.into_iter().map(UserResponse::from).collect(),
            pagination,
            total,
        ))
    }
}
