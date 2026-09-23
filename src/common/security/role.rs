use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum Role {
    #[default]
    User,
    Admin,
}

impl Role {
    pub fn is_admin(self) -> bool {
        self == Role::Admin
    }
}
