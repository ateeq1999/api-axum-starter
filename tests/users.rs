mod common;

use axum::http::StatusCode;
use common::{PASSWORD, spawn};
use serde_json::json;

#[tokio::test]
async fn user_routes_require_authentication_and_admin_role() {
    let app = spawn().await;
    let (_, user) = app.user("user@example.com").await;

    assert_eq!(
        app.get("/api/v1/users", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get("/api/v1/users", Some(&user)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.post(
            "/api/v1/users",
            Some(&user),
            json!({ "email": "x@example.com", "password": PASSWORD })
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.get("/api/v1/users", Some("garbage")).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn admin_creates_users_and_lists_them_with_pagination_and_search() {
    let app = spawn().await;
    let (_, admin) = app.admin("root@example.com").await;

    for name in ["anna", "bert", "cora"] {
        let (status, body) = app
            .post(
                "/api/v1/users",
                Some(&admin),
                json!({ "email": format!("{name}@example.com"), "password": PASSWORD, "display_name": name }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["role"], "user");
        assert!(body.get("password_hash").is_none());
    }

    let (status, dup) = app
        .post(
            "/api/v1/users",
            Some(&admin),
            json!({ "email": "ANNA@example.com", "password": PASSWORD }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{dup}");

    let (status, page) = app
        .get(
            "/api/v1/users?per_page=2&page=1&sort=email&order=asc",
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["total"], 4);
    assert_eq!(page["per_page"], 2);
    let emails: Vec<_> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["email"].as_str().unwrap())
        .collect();
    assert_eq!(emails, ["anna@example.com", "bert@example.com"]);

    let (_, page2) = app
        .get(
            "/api/v1/users?per_page=2&page=2&sort=email&order=asc",
            Some(&admin),
        )
        .await;
    assert_eq!(page2["items"].as_array().unwrap().len(), 2);

    let (_, found) = app.get("/api/v1/users?q=cor", Some(&admin)).await;
    assert_eq!(found["total"], 1);
    assert_eq!(found["items"][0]["display_name"], "cora");

    // LIKE wildcards in the search text are literal
    let (_, none) = app.get("/api/v1/users?q=%25", Some(&admin)).await;
    assert_eq!(none["total"], 0);

    let (status, _) = app
        .get("/api/v1/users?sort=password_hash", Some(&admin))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn users_can_read_and_update_only_themselves() {
    let app = spawn().await;
    let (alice_id, alice) = app.user("alice@example.com").await;
    let (bob_id, _) = app.user("bob@example.com").await;

    let (status, body) = app
        .get(&format!("/api/v1/users/{alice_id}"), Some(&alice))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], "alice@example.com");

    let (status, _) = app
        .get(&format!("/api/v1/users/{bob_id}"), Some(&alice))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, me) = app
        .patch(
            "/api/v1/users/me",
            Some(&alice),
            json!({ "display_name": "Alice A." }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["display_name"], "Alice A.");

    // role and active state are admin-only
    let (status, _) = app
        .patch(
            &format!("/api/v1/users/{alice_id}"),
            Some(&alice),
            json!({ "role": "admin" }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = app
        .patch(
            "/api/v1/users/me",
            Some(&alice),
            json!({ "display_name": "" }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn admin_updates_role_and_active_state() {
    let app = spawn().await;
    let (admin_id, admin) = app.admin("root@example.com").await;
    let (user_id, _) = app.user("user@example.com").await;

    let (status, body) = app
        .patch(
            &format!("/api/v1/users/{user_id}"),
            Some(&admin),
            json!({ "role": "admin" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["role"], "admin");

    let (status, body) = app
        .patch(
            &format!("/api/v1/users/{user_id}"),
            Some(&admin),
            json!({ "is_active": false }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["is_active"], false);
    assert_eq!(
        app.login("user@example.com", PASSWORD).await.0,
        StatusCode::FORBIDDEN
    );

    // an admin cannot lock themselves out
    let (status, _) = app
        .patch(
            &format!("/api/v1/users/{admin_id}"),
            Some(&admin),
            json!({ "role": "user" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = app
        .patch(
            &format!("/api/v1/users/{admin_id}"),
            Some(&admin),
            json!({ "is_active": false }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = app
        .patch(
            "/api/v1/users/00000000-0000-0000-0000-000000000000",
            Some(&admin),
            json!({ "is_active": true }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_last_active_admin_cannot_be_removed() {
    let app = spawn().await;
    let (a_id, a_token) = app.admin("a@example.com").await;
    let (b_id, b_token) = app.admin("b@example.com").await;

    // A deactivates B; B still holds a valid token but A is now the only active admin.
    let (status, _) = app
        .patch(
            &format!("/api/v1/users/{b_id}"),
            Some(&a_token),
            json!({ "is_active": false }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = app
        .patch(
            &format!("/api/v1/users/{a_id}"),
            Some(&b_token),
            json!({ "role": "user" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, _) = app
        .delete(&format!("/api/v1/users/{a_id}"), Some(&b_token))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn soft_delete_hides_the_user_and_frees_the_email() {
    let app = spawn().await;
    let (admin_id, admin) = app.admin("root@example.com").await;
    let (user_id, user) = app.user("gone@example.com").await;

    let (status, _) = app
        .delete(&format!("/api/v1/users/{admin_id}"), Some(&admin))
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "admins cannot delete themselves"
    );

    let (status, _) = app
        .delete(&format!("/api/v1/users/{user_id}"), Some(&admin))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_eq!(
        app.get(&format!("/api/v1/users/{user_id}"), Some(&admin))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.delete(&format!("/api/v1/users/{user_id}"), Some(&admin))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.login("gone@example.com", PASSWORD).await.0,
        StatusCode::UNAUTHORIZED
    );
    // the deleted user's old token no longer resolves to anyone
    assert_eq!(
        app.get("/api/v1/users/me", Some(&user)).await.0,
        StatusCode::NOT_FOUND
    );

    let (_, page) = app.get("/api/v1/users", Some(&admin)).await;
    assert_eq!(page["total"], 1);

    // the address can be registered again
    app.register("gone@example.com").await;
}

#[tokio::test]
async fn bootstrap_admin_is_created_once() {
    let app = spawn().await;
    app.state
        .users
        .ensure_bootstrap_admin("Boss@Example.com", PASSWORD.to_string())
        .await
        .unwrap();
    // second call is a no-op even with different credentials
    app.state
        .users
        .ensure_bootstrap_admin("other@example.com", PASSWORD.to_string())
        .await
        .unwrap();

    let token = app.login_token("boss@example.com").await;
    let (_, me) = app.get("/api/v1/users/me", Some(&token)).await;
    assert_eq!(me["role"], "admin");
    assert_eq!(me["email_verified"], true);
    assert_eq!(
        app.login("other@example.com", PASSWORD).await.0,
        StatusCode::UNAUTHORIZED
    );
}
