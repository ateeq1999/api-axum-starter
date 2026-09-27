//! The admin user list: search, sort, pagination.

use sqlx::{Postgres, QueryBuilder};

use super::{UserFilter, UsersRepository};
use crate::{
    common::error::AppResult,
    modules::users::{
        dto::{SortOrder, UserSort},
        entity::User,
    },
};

impl UsersRepository {
    pub async fn list(&self, filter: &UserFilter<'_>) -> AppResult<(Vec<User>, u64)> {
        let pattern = filter.search.map(|q| format!("%{}%", escape_like(q)));

        let mut count =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL");
        push_search(&mut count, pattern.as_deref());
        let total: i64 = count.build_query_scalar().fetch_one(&self.db).await?;

        let column = match filter.sort {
            UserSort::CreatedAt => "created_at",
            UserSort::Email => "email",
        };
        let direction = match filter.order {
            SortOrder::Asc => "ASC",
            SortOrder::Desc => "DESC",
        };

        let mut select = QueryBuilder::<Postgres>::new(concat!(
            "SELECT ",
            columns!(),
            " FROM users WHERE deleted_at IS NULL"
        ));
        push_search(&mut select, pattern.as_deref());
        // `column` and `direction` come from closed enums, never from user text.
        select.push(format!(
            " ORDER BY {column} {direction}, id {direction} LIMIT "
        ));
        select.push_bind(filter.limit);
        select.push(" OFFSET ");
        select.push_bind(filter.offset);
        let users = select.build_query_as::<User>().fetch_all(&self.db).await?;

        Ok((users, total.max(0) as u64))
    }
}

fn push_search(qb: &mut QueryBuilder<Postgres>, pattern: Option<&str>) {
    if let Some(pattern) = pattern {
        qb.push(" AND (email LIKE ")
            .push_bind(pattern.to_string())
            .push(" ESCAPE '\\' OR display_name LIKE ")
            .push_bind(pattern.to_string())
            .push(" ESCAPE '\\')");
    }
}

fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_wildcards() {
        assert_eq!(escape_like("50%_off\\"), "50\\%\\_off\\\\");
    }
}
