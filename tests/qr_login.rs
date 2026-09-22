mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, spawn};
use serde_json::{Value, json};

/// The device that wants to sign in (a browser showing the QR code).
struct NewDevice {
    id: String,
    secret: String,
    code: String,
}

async fn new_device(app: &TestApp) -> NewDevice {
    let (status, body) = app
        .json_with(
            Method::POST,
            "/api/v1/auth/qr/sessions",
            &[("user-agent", "Mozilla/5.0 (Windows NT 10.0) Firefox/130")],
            None,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    NewDevice {
        id: body["id"].as_str().unwrap().to_string(),
        secret: body["poll_secret"].as_str().unwrap().to_string(),
        code: body["verification_code"].as_str().unwrap().to_string(),
    }
}

async fn poll(app: &TestApp, device: &NewDevice) -> (StatusCode, Value) {
    app.json_with(
        Method::GET,
        &format!("/api/v1/auth/qr/sessions/{}", device.id),
        &[("x-qr-secret", &device.secret)],
        None,
    )
    .await
}

fn path(device: &NewDevice, action: &str) -> String {
    format!("/api/v1/auth/qr/sessions/{}/{action}", device.id)
}

#[tokio::test]
async fn the_whatsapp_style_flow() {
    let app = spawn().await;
    let (user_id, phone) = app.user("phone@example.com").await;

    // 1. the browser asks for a QR code
    let (status, created) = app.post("/api/v1/auth/qr/sessions", None, json!({})).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();
    assert_eq!(
        created["qr_payload"],
        format!("https://app.test/qr-login?session={id}")
    );
    assert!(created["qr_svg"].as_str().unwrap().contains("<svg"));
    assert_eq!(created["verification_code"].as_str().unwrap().len(), 4);
    assert!(created["poll_secret"].as_str().unwrap().len() >= 40);
    assert!(
        !created["qr_payload"]
            .as_str()
            .unwrap()
            .contains(created["poll_secret"].as_str().unwrap())
    );
    assert_eq!(created["poll_interval_secs"], 2);
    let device = NewDevice {
        id: id.to_string(),
        secret: created["poll_secret"].as_str().unwrap().to_string(),
        code: created["verification_code"].as_str().unwrap().to_string(),
    };
    assert_eq!(poll(&app, &device).await.1["status"], "pending");

    // 2. the phone scans it and sees where the request came from
    let (status, scan) = app
        .post(&path(&device, "scan"), Some(&phone), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{scan}");
    assert!(scan["requester_agent"].as_str().unwrap_or("").len() <= 200);
    assert!(scan["requested_at"].is_string() && scan["expires_at"].is_string());
    assert_eq!(poll(&app, &device).await.1["status"], "scanned");

    // 3. wrong code, then the right one
    let (status, body) = app
        .post(
            &path(&device, "approve"),
            Some(&phone),
            json!({ "code": "0000" }),
        )
        .await;
    if device.code != "0000" {
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(poll(&app, &device).await.1["status"], "scanned");
    }
    let (status, body) = app
        .post(
            &path(&device, "approve"),
            Some(&phone),
            json!({ "code": device.code }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // 4. the browser collects its token, once, and it belongs to the phone's user
    let (status, done) = poll(&app, &device).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(done["status"], "approved");
    assert_eq!(done["token_type"], "Bearer");
    let (_, me) = app
        .get(
            "/api/v1/users/me",
            Some(done["access_token"].as_str().unwrap()),
        )
        .await;
    assert_eq!(me["id"], user_id.as_str());

    let (_, again) = poll(&app, &device).await;
    assert_eq!(again["status"], "consumed");
    assert!(again.get("access_token").is_none());
}

#[tokio::test]
async fn the_request_origin_is_shown_to_the_approver() {
    let app = spawn().await;
    let (_, phone) = app.user("who@example.com").await;
    let device = new_device(&app).await;

    let (_, scan) = app
        .post(&path(&device, "scan"), Some(&phone), json!({}))
        .await;
    assert_eq!(
        scan["requester_agent"],
        "Mozilla/5.0 (Windows NT 10.0) Firefox/130"
    );
}

#[tokio::test]
async fn only_the_creating_device_can_poll() {
    let app = spawn().await;
    let device = new_device(&app).await;

    let no_secret = app
        .get(&format!("/api/v1/auth/qr/sessions/{}", device.id), None)
        .await;
    assert_eq!(no_secret.0, StatusCode::NOT_FOUND);
    let wrong = NewDevice {
        id: device.id.clone(),
        secret: "not-the-secret".into(),
        code: String::new(),
    };
    assert_eq!(poll(&app, &wrong).await.0, StatusCode::NOT_FOUND);
    let unknown = NewDevice {
        id: "does-not-exist".into(),
        secret: device.secret.clone(),
        code: String::new(),
    };
    assert_eq!(poll(&app, &unknown).await.0, StatusCode::NOT_FOUND);
    // the id alone (which is in the QR code) is not enough
    assert_eq!(poll(&app, &device).await.1["status"], "pending");
}

#[tokio::test]
async fn rejecting_ends_the_session() {
    let app = spawn().await;
    let (_, phone) = app.user("no@example.com").await;
    let device = new_device(&app).await;

    app.post(&path(&device, "scan"), Some(&phone), json!({}))
        .await;
    let (status, _) = app
        .post(&path(&device, "reject"), Some(&phone), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(poll(&app, &device).await.1["status"], "rejected");

    // a rejected session cannot be approved afterwards
    let (status, _) = app
        .post(
            &path(&device, "approve"),
            Some(&phone),
            json!({ "code": device.code }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(poll(&app, &device).await.1["status"], "rejected");
}

#[tokio::test]
async fn guessing_the_verification_code_cancels_the_login() {
    let app = spawn().await;
    let (_, phone) = app.user("guess@example.com").await;
    let device = new_device(&app).await;
    app.post(&path(&device, "scan"), Some(&phone), json!({}))
        .await;

    let wrong = if device.code == "1234" {
        "4321"
    } else {
        "1234"
    };
    let mut last = StatusCode::OK;
    for _ in 0..3 {
        last = app
            .post(
                &path(&device, "approve"),
                Some(&phone),
                json!({ "code": wrong }),
            )
            .await
            .0;
        assert_eq!(last, StatusCode::BAD_REQUEST);
    }
    assert_eq!(last, StatusCode::BAD_REQUEST);
    assert_eq!(poll(&app, &device).await.1["status"], "rejected");

    // even the right code no longer works
    let (status, _) = app
        .post(
            &path(&device, "approve"),
            Some(&phone),
            json!({ "code": device.code }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = app
        .post(
            &path(&device, "approve"),
            Some(&phone),
            json!({ "code": "12" }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}

#[tokio::test]
async fn only_the_scanning_user_can_approve_and_only_one_can_scan() {
    let app = spawn().await;
    let (_, alice) = app.user("alice@example.com").await;
    let (_, mallory) = app.user("mallory@example.com").await;
    let device = new_device(&app).await;

    // approving before anyone scanned is not possible
    let (status, _) = app
        .post(
            &path(&device, "approve"),
            Some(&alice),
            json!({ "code": device.code }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    assert_eq!(
        app.post(&path(&device, "scan"), Some(&alice), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    // scanning again by the same user is fine, by another is a conflict
    assert_eq!(
        app.post(&path(&device, "scan"), Some(&alice), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        app.post(&path(&device, "scan"), Some(&mallory), json!({}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    // and Mallory cannot approve or reject Alice's login even with the right code
    let (status, _) = app
        .post(
            &path(&device, "approve"),
            Some(&mallory),
            json!({ "code": device.code }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        app.post(&path(&device, "reject"), Some(&mallory), json!({}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(poll(&app, &device).await.1["status"], "scanned");
}

#[tokio::test]
async fn expired_sessions_are_dead() {
    let app = spawn().await;
    let (_, phone) = app.user("late@example.com").await;
    let device = new_device(&app).await;
    app.post(&path(&device, "scan"), Some(&phone), json!({}))
        .await;
    app.post(
        &path(&device, "approve"),
        Some(&phone),
        json!({ "code": device.code }),
    )
    .await;

    // approved, but the browser did not collect in time
    sqlx::query("UPDATE qr_sessions SET expires_at = $1")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1))
        .execute(&app.state.db)
        .await
        .unwrap();
    let (status, body) = poll(&app, &device).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "expired");
    assert!(body.get("access_token").is_none());
    assert_eq!(
        app.post(&path(&device, "scan"), Some(&phone), json!({}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn approving_requires_an_interactive_session() {
    let app = spawn().await;
    let (_, session) = app.user("keys@example.com").await;
    let key = app
        .post(
            "/api/v1/api-keys",
            Some(&session),
            json!({ "name": "k", "scope": "write" }),
        )
        .await
        .1["key"]
        .as_str()
        .unwrap()
        .to_string();
    let device = new_device(&app).await;

    for action in ["scan", "reject"] {
        assert_eq!(
            app.post(&path(&device, action), None, json!({})).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.post(&path(&device, action), Some(&key), json!({}))
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let body = json!({ "code": device.code });
    assert_eq!(
        app.post(&path(&device, "approve"), Some(&key), body)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn a_disabled_account_cannot_collect_a_token() {
    let app = spawn().await;
    let (_, phone) = app.user("disabled@example.com").await;
    let device = new_device(&app).await;
    app.post(&path(&device, "scan"), Some(&phone), json!({}))
        .await;
    app.post(
        &path(&device, "approve"),
        Some(&phone),
        json!({ "code": device.code }),
    )
    .await;

    sqlx::query("UPDATE users SET is_active = FALSE")
        .execute(&app.state.db)
        .await
        .unwrap();
    let (status, body) = poll(&app, &device).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn polling_has_its_own_higher_rate_limit() {
    let mut config = common::test_config();
    config.account.rate_limit_per_minute = 3;
    config.qr_login.poll_rate_limit_per_minute = 50;
    let app = common::spawn_with(config).await;

    let device = new_device(&app).await; // uses 1 of the 3 strict slots
    for _ in 0..20 {
        assert_eq!(
            poll(&app, &device).await.0,
            StatusCode::OK,
            "polling is not held to the strict limit"
        );
    }
    // creating sessions is held to the strict limit
    assert_eq!(
        app.post("/api/v1/auth/qr/sessions", None, json!({}))
            .await
            .0,
        StatusCode::CREATED
    );
    assert_eq!(
        app.post("/api/v1/auth/qr/sessions", None, json!({}))
            .await
            .0,
        StatusCode::CREATED
    );
    assert_eq!(
        app.post("/api/v1/auth/qr/sessions", None, json!({}))
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
}
