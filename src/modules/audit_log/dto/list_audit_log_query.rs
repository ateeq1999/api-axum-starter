use serde::Deserialize;
use utoipa::IntoParams;
use validator::Validate;

use crate::common::dto::Pagination;

#[derive(Debug, Deserialize, Validate, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListAuditLogQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl ListAuditLogQuery {
    pub fn pagination(&self) -> Pagination {
        Pagination::new(self.page, self.per_page)
    }
}
