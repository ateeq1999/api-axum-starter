mod common;

use api_starter_axum::infra::seed;
use axum::http::StatusCode;
use serde_json::json;

const SEED_PASSWORD: &str = "Password123!";

#[tokio::test]
async fn seed_is_idempotent() {
    let pool = common::fresh_pool().await;
    assert_eq!(seed::run(&pool).await.unwrap(), 5);
    assert_eq!(seed::run(&pool).await.unwrap(), 5);

    let admins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(admins, 1);
}

/// The seeded rows must be readable by the application (ids are stored as blobs) and log in.
#[tokio::test]
async fn seeded_accounts_work_through_the_api() {
    let app = common::spawn().await;
    seed::run(&app.state.db).await.unwrap();

    let (status, body) = app.login("admin@example.com", SEED_PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let admin = body["access_token"].as_str().unwrap().to_string();

    let (status, page) = app
        .get("/api/v1/users?sort=email&order=asc", Some(&admin))
        .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 5);
    assert_eq!(page["items"][0]["email"], "admin@example.com");
    assert_eq!(page["items"][0]["role"], "admin");

    // verified user logs in; unverified one can too (REQUIRE_VERIFIED_EMAIL is off in tests)
    assert_eq!(
        app.login("alice@example.com", SEED_PASSWORD).await.0,
        StatusCode::OK
    );
    let (_, carol) = app.login("carol@example.com", SEED_PASSWORD).await;
    let carol = carol["access_token"].as_str().unwrap().to_string();
    let (_, me) = app.get("/api/v1/users/me", Some(&carol)).await;
    assert_eq!(me["email_verified"], false);

    // the deactivated account is rejected even with the right password
    let (status, _) = app
        .post(
            "/api/v1/auth/login",
            None,
            json!({ "email": "dave@example.com", "password": SEED_PASSWORD }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn fresh_seed_removes_everything_except_the_seed_data() {
    let app = common::spawn().await;
    seed::run(&app.state.db).await.unwrap();

    // extra data that a fresh seed must remove: a user and its emailed-link tokens
    app.register("extra@example.com").await;
    let tokens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM auth_tokens")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert!(
        tokens > 0,
        "registration should have created a verification token"
    );

    assert_eq!(seed::fresh(&app.state.db).await.unwrap(), 5);

    let extra = app.login("extra@example.com", common::PASSWORD).await;
    assert_eq!(extra.0, StatusCode::UNAUTHORIZED);
    let tokens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM auth_tokens")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(tokens, 0);
    assert_eq!(
        app.login("admin@example.com", SEED_PASSWORD).await.0,
        StatusCode::OK
    );
}
