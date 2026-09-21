use serde::Deserialize;
use validator::Validate;

use crate::common::dto::Pagination;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserSort {
    #[default]
    CreatedAt,
    Email,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ListUsersQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    #[validate(length(max = 100, message = "must be at most 100 characters"))]
    pub q: Option<String>,
    pub sort: Option<UserSort>,
    pub order: Option<SortOrder>,
}

impl ListUsersQuery {
    pub fn pagination(&self) -> Pagination {
        Pagination::new(self.page, self.per_page)
    }
}
