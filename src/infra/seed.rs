use sqlx::SqlitePool;

/// Development seed data, embedded at compile time.
const SEED_SQL: &str = include_str!("../../seeds/seed.sql");

/// Runs `seeds/seed.sql` and returns how many users exist afterwards.
/// The script is idempotent, so running it again changes nothing.
pub async fn run(db: &SqlitePool) -> anyhow::Result<i64> {
    sqlx::raw_sql(SEED_SQL).execute(db).await?;
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(db)
        .await?)
}
