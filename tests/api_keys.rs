mod common;

use axum::http::{Method, StatusCode};
use common::spawn;
use serde_json::json;

async fn create_key(app: &common::TestApp, session: &str, scope: &str) -> (String, String) {
    let (status, body) = app
        .post(
            "/api/v1/api-keys",
            Some(session),
            json!({ "name": "ci", "scope": scope }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (
        body["key"].as_str().unwrap().to_string(),
        body["id"].as_str().unwrap().to_string(),
    )
}

#[tokio::test]
async fn a_key_authenticates_as_its_owner() {
    let app = spawn().await;
    let (_, session) = app.user("keys@example.com").await;

    let (status, created) = app
        .post(
            "/api/v1/api-keys",
            Some(&session),
            json!({ "name": "  laptop  ", "scope": "write", "expires_in_days": 30 }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let key = created["key"].as_str().unwrap();
    assert!(key.starts_with("ak_") && key.len() > 40);
    assert_eq!(created["name"], "laptop");
    assert_eq!(created["scope"], "write");
    assert!(key.starts_with(created["key_prefix"].as_str().unwrap()));
    assert!(created["expires_at"].is_string());

    // as a Bearer token and as X-API-Key
    let (status, me) = app.get("/api/v1/users/me", Some(key)).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["email"], "keys@example.com");
    let (status, me) = app
        .json_with(Method::GET, "/api/v1/users/me", &[("x-api-key", key)], None)
        .await;
    assert_eq!(status, StatusCode::OK, "{me}");

    // the secret is stored only as a hash and never listed again
    let stored: String = sqlx::query_scalar("SELECT key_hash FROM api_keys")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert!(!stored.contains(key) && stored.len() == 64);
    let (_, list) = app.get("/api/v1/api-keys", Some(&session)).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert!(list[0].get("key").is_none());
    assert!(list[0]["last_used_at"].is_string(), "use is recorded");
}

#[tokio::test]
async fn read_only_keys_cannot_change_anything() {
    let app = spawn().await;
    let (_, session) = app.user("ro@example.com").await;
    let (read_key, _) = create_key(&app, &session, "read").await;
    let (write_key, _) = create_key(&app, &session, "write").await;

    assert_eq!(
        app.get("/api/v1/users/me", Some(&read_key)).await.0,
        StatusCode::OK
    );
    let patch = json!({ "display_name": "Changed" });
    let (status, body) = app
        .patch("/api/v1/users/me", Some(&read_key), patch.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("read-only")
    );

    let (status, me) = app.patch("/api/v1/users/me", Some(&write_key), patch).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["display_name"], "Changed");
}

#[tokio::test]
async fn keys_cannot_manage_credentials() {
    let app = spawn().await;
    let (_, session) = app.user("cred@example.com").await;
    let (key, _) = create_key(&app, &session, "write").await;

    // A leaked key must not be able to take the account over or mint more keys.
    let calls = [
        app.post("/api/v1/api-keys", Some(&key), json!({ "name": "x" }))
            .await,
        app.get("/api/v1/api-keys", Some(&key)).await,
        app.post(
            "/api/v1/auth/password/change",
            Some(&key),
            json!({ "current_password": common::PASSWORD, "new_password": "brand-new-password" }),
        )
        .await,
        app.post(
            "/api/v1/auth/email/change",
            Some(&key),
            json!({ "new_email": "x@example.com", "current_password": common::PASSWORD }),
        )
        .await,
        app.post(
            "/api/v1/auth/passkeys/register/begin",
            Some(&key),
            json!({}),
        )
        .await,
        app.get("/api/v1/auth/oauth/identities", Some(&key)).await,
    ];
    for (status, body) in calls {
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("interactive")
        );
    }
}

#[tokio::test]
async fn revoked_expired_and_unknown_keys_are_rejected() {
    let app = spawn().await;
    let (_, session) = app.user("gone@example.com").await;
    let (key, id) = create_key(&app, &session, "read").await;
    assert_eq!(
        app.get("/api/v1/users/me", Some(&key)).await.0,
        StatusCode::OK
    );

    // revoke
    let (status, _) = app
        .delete(&format!("/api/v1/api-keys/{id}"), Some(&session))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = app.get("/api/v1/users/me", Some(&key)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(
        app.delete(&format!("/api/v1/api-keys/{id}"), Some(&session))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, list) = app.get("/api/v1/api-keys", Some(&session)).await;
    assert!(list.as_array().unwrap().is_empty());

    // expiry
    let (short, _) = create_key(&app, &session, "read").await;
    sqlx::query("UPDATE api_keys SET expires_at = $1 WHERE revoked_at IS NULL")
        .bind(chrono::Utc::now() - chrono::Duration::minutes(1))
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_eq!(
        app.get("/api/v1/users/me", Some(&short)).await.0,
        StatusCode::UNAUTHORIZED
    );

    // unknown and malformed
    for bad in ["ak_doesnotexist", "ak_", "ak_ ", "Bearer"] {
        assert_eq!(
            app.get("/api/v1/users/me", Some(bad)).await.0,
            StatusCode::UNAUTHORIZED,
            "{bad}"
        );
    }
}

#[tokio::test]
async fn keys_follow_their_owner_immediately() {
    let app = spawn().await;
    let (_, admin) = app.admin("boss@example.com").await;
    let (user_id, user_session) = app.user("worker@example.com").await;
    let (key, _) = create_key(&app, &user_session, "write").await;
    assert_eq!(
        app.get("/api/v1/users/me", Some(&key)).await.0,
        StatusCode::OK
    );

    // An admin key acts with admin rights, read from the database on every request.
    let (admin_key, _) = create_key(&app, &admin, "read").await;
    assert_eq!(
        app.get("/api/v1/users", Some(&admin_key)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        app.get("/api/v1/users", Some(&key)).await.0,
        StatusCode::FORBIDDEN
    );

    // Deactivating the owner kills the key at once (a JWT would live until it expires).
    let (status, _) = app
        .patch(
            &format!("/api/v1/users/{user_id}"),
            Some(&admin),
            json!({ "is_active": false }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        app.get("/api/v1/users/me", Some(&key)).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn users_only_see_and_revoke_their_own_keys_and_are_limited() {
    let app = spawn().await;
    let (_, alice) = app.user("alice@example.com").await;
    let (_, bob) = app.user("bob@example.com").await;
    let (_, alice_key_id) = create_key(&app, &alice, "read").await;

    let (_, bobs) = app.get("/api/v1/api-keys", Some(&bob)).await;
    assert!(bobs.as_array().unwrap().is_empty());
    assert_eq!(
        app.delete(&format!("/api/v1/api-keys/{alice_key_id}"), Some(&bob))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    let (status, body) = app
        .post("/api/v1/api-keys", Some(&alice), json!({ "name": "" }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    for _ in 0..19 {
        create_key(&app, &alice, "read").await;
    }
    let (status, body) = app
        .post(
            "/api/v1/api-keys",
            Some(&alice),
            json!({ "name": "one too many" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("at most 20")
    );
}
