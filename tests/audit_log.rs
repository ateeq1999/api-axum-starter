//! `modules::audit_log`: administrative actions on users are recorded and listable by admins.

mod common;

use axum::http::StatusCode;
use common::spawn;
use serde_json::json;

#[tokio::test]
async fn admin_creating_a_user_is_recorded() {
    let app = spawn().await;
    let (_, admin) = app.admin("root@example.com").await;

    let (status, created) = app
        .post(
            "/api/v1/users",
            Some(&admin),
            json!({ "email": "new.hire@example.com", "password": common::PASSWORD, "role": "admin" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let (status, page) = app.get("/api/v1/audit-log", Some(&admin)).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let entry = &page["items"][0];
    assert_eq!(entry["action"], "user.created_by_admin");
    assert_eq!(entry["target_user_id"], created["id"]);
    assert_eq!(entry["details"]["email"], "new.hire@example.com");
    assert_eq!(entry["details"]["role"], "admin");
}

#[tokio::test]
async fn role_and_active_status_changes_are_recorded_separately() {
    let app = spawn().await;
    let (_, admin) = app.admin("root@example.com").await;
    let (target_id, _) = app.user("member@example.com").await;

    let path = format!("/api/v1/users/{target_id}");
    let (status, _) = app
        .patch(
            &path,
            Some(&admin),
            json!({ "role": "admin", "is_active": false }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (_, page) = app.get("/api/v1/audit-log", Some(&admin)).await;
    let actions: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["action"].as_str().unwrap())
        .collect();
    assert!(actions.contains(&"user.role_changed"), "{actions:?}");
    assert!(
        actions.contains(&"user.active_status_changed"),
        "{actions:?}"
    );

    let role_change = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["action"] == "user.role_changed")
        .unwrap();
    assert_eq!(role_change["details"]["from"], "user");
    assert_eq!(role_change["details"]["to"], "admin");
}

#[tokio::test]
async fn a_display_name_only_update_records_nothing() {
    let app = spawn().await;
    let (_, admin) = app.admin("root@example.com").await;
    let (target_id, _) = app.user("member@example.com").await;

    let path = format!("/api/v1/users/{target_id}");
    let (status, _) = app
        .patch(&path, Some(&admin), json!({ "display_name": "New Name" }))
        .await;
    assert_eq!(status, StatusCode::OK);

    let (_, page) = app.get("/api/v1/audit-log", Some(&admin)).await;
    assert_eq!(page["items"], json!([]));
}

#[tokio::test]
async fn deleting_a_user_is_recorded_with_their_email() {
    let app = spawn().await;
    let (_, admin) = app.admin("root@example.com").await;
    let (target_id, _) = app.user("gone@example.com").await;

    let (status, _) = app
        .delete(&format!("/api/v1/users/{target_id}"), Some(&admin))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, page) = app.get("/api/v1/audit-log", Some(&admin)).await;
    let entry = &page["items"][0];
    assert_eq!(entry["action"], "user.deleted");
    assert_eq!(entry["details"]["email"], "gone@example.com");
}

#[tokio::test]
async fn only_admins_can_read_the_audit_log() {
    let app = spawn().await;
    let (_, token) = app.user("member@example.com").await;

    let (status, _) = app.get("/api/v1/audit-log", Some(&token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = app.get("/api/v1/audit-log", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
