use sqlx::PgPool;

macro_rules! identity_columns {
    () => {
        "id, user_id, provider, provider_user_id, email, created_at"
    };
}

mod grants;
mod identities;
mod states;

#[derive(Clone)]
pub struct OAuthRepository {
    db: PgPool,
}

impl OAuthRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}
