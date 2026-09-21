use serde::Serialize;

const DEFAULT_PER_PAGE: u32 = 20;
pub const MAX_PER_PAGE: u32 = 100;

/// Validated page window. Build it from the raw optional query values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
}

impl Pagination {
    pub fn new(page: Option<u32>, per_page: Option<u32>) -> Self {
        Self {
            page: page.unwrap_or(1).max(1),
            per_page: per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE),
        }
    }

    pub fn limit(&self) -> i64 {
        i64::from(self.per_page)
    }

    pub fn offset(&self) -> i64 {
        i64::from(self.page - 1) * i64::from(self.per_page)
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
}

impl<T> PaginatedResponse<T> {
    pub fn new(items: Vec<T>, pagination: Pagination, total: u64) -> Self {
        Self {
            items,
            page: pagination.page,
            per_page: pagination.per_page,
            total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_clamps() {
        assert_eq!(
            Pagination::new(None, None),
            Pagination {
                page: 1,
                per_page: 20
            }
        );
        let p = Pagination::new(Some(0), Some(10_000));
        assert_eq!((p.page, p.per_page), (1, MAX_PER_PAGE));
    }

    #[test]
    fn offset_is_zero_based() {
        let p = Pagination::new(Some(3), Some(25));
        assert_eq!((p.limit(), p.offset()), (25, 50));
    }
}
