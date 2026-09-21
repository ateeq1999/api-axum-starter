use sqlx::SqlitePool;

/// Development seed data, embedded at compile time.
const SEED_SQL: &str = include_str!("../../seeds/seed.sql");
/// Deletes all application data (the schema stays).
const RESET_SQL: &str = include_str!("../../seeds/reset.sql");

/// Runs `seeds/seed.sql` and returns how many users exist afterwards.
/// The script is idempotent, so running it again changes nothing.
pub async fn run(db: &SqlitePool) -> anyhow::Result<i64> {
    sqlx::raw_sql(SEED_SQL).execute(db).await?;
    user_count(db).await
}

/// Runs `seeds/reset.sql`: deletes every user and token. Destructive.
pub async fn reset(db: &SqlitePool) -> anyhow::Result<()> {
    sqlx::raw_sql(RESET_SQL).execute(db).await?;
    Ok(())
}

/// Wipes the database, then loads the seed data: afterwards it holds exactly the seed rows.
pub async fn fresh(db: &SqlitePool) -> anyhow::Result<i64> {
    reset(db).await?;
    run(db).await
}

async fn user_count(db: &SqlitePool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(db)
        .await?)
}
