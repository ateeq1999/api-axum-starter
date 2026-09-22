use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
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
