use sqlx::PgPool;

macro_rules! columns {
    () => {
        "id, user_id, name, credential_id, credential_json, created_at, last_used_at"
    };
}

mod challenges;
mod credentials;

#[derive(Clone)]
pub struct PasskeysRepository {
    db: PgPool,
}

impl PasskeysRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}
