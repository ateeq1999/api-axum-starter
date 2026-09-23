//! End-to-end TOTP two-factor flow: enroll, a password-only login is no longer enough, a code
//! (or a recovery code) from the authenticator finishes it, and disabling requires the password.

mod common;

use axum::http::StatusCode;
use common::{PASSWORD, spawn};
use serde_json::json;
use totp_rs::{Algorithm, Secret, TOTP};

/// Computes the current valid code for a base32 secret the same way the server does
/// (`common::security::totp`), so the test can act as the authenticator app would.
fn current_code_for(base32_secret: &str) -> String {
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        Secret::Encoded(base32_secret.to_string())
            .to_bytes()
            .unwrap(),
        None,
        "test".to_string(),
    )
    .unwrap();
    totp.generate_current().unwrap()
}

#[tokio::test]
async fn enrolling_then_logging_in_requires_the_authenticator_code() {
    let app = spawn().await;
    let (_, token) = app.user("2fa@example.com").await;

    let (status, setup) = app
        .post("/api/v1/auth/2fa/setup", Some(&token), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{setup}");
    let secret = setup["secret"].as_str().unwrap();
    assert!(
        setup["qr_code_data_uri"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );

    let (status, enabled) = app
        .post(
            "/api/v1/auth/2fa/enable",
            Some(&token),
            json!({ "code": current_code_for(secret) }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{enabled}");
    let recovery_codes: Vec<String> = enabled["recovery_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    assert_eq!(recovery_codes.len(), 8);

    // The password alone is no longer enough.
    let (status, login) = app.login("2fa@example.com", PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{login}");
    assert_eq!(login["requires_totp"], true);
    assert!(login.get("access_token").is_none());
    let pending_token = login["pending_token"].as_str().unwrap();

    // A wrong code does not complete it.
    let (status, _) = app
        .post(
            "/api/v1/auth/2fa/verify",
            None,
            json!({ "pending_token": pending_token, "code": "000000" }),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, verified) = app
        .post(
            "/api/v1/auth/2fa/verify",
            None,
            json!({ "pending_token": pending_token, "code": current_code_for(secret) }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{verified}");
    assert!(verified["access_token"].as_str().is_some());

    // The pending token is single-use: reusing it (even with a valid code) is now simply an
    // invalid/expired token, the same rejection any other spent one-time token gets.
    let (status, _) = app
        .post(
            "/api/v1/auth/2fa/verify",
            None,
            json!({ "pending_token": pending_token, "code": current_code_for(secret) }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_recovery_code_works_once_instead_of_the_authenticator() {
    let app = spawn().await;
    let (_, token) = app.user("recover@example.com").await;
    let (_, setup) = app
        .post("/api/v1/auth/2fa/setup", Some(&token), json!({}))
        .await;
    let secret = setup["secret"].as_str().unwrap();
    let (_, enabled) = app
        .post(
            "/api/v1/auth/2fa/enable",
            Some(&token),
            json!({ "code": current_code_for(secret) }),
        )
        .await;
    let recovery_code = enabled["recovery_codes"][0].as_str().unwrap();

    let (_, login) = app.login("recover@example.com", PASSWORD).await;
    let pending_token = login["pending_token"].as_str().unwrap();

    let (status, verified) = app
        .post(
            "/api/v1/auth/2fa/verify",
            None,
            json!({ "pending_token": pending_token, "code": recovery_code }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{verified}");

    // Using the same recovery code again (with a fresh login attempt) fails: one use each.
    let (_, login2) = app.login("recover@example.com", PASSWORD).await;
    let pending_token2 = login2["pending_token"].as_str().unwrap();
    let (status, _) = app
        .post(
            "/api/v1/auth/2fa/verify",
            None,
            json!({ "pending_token": pending_token2, "code": recovery_code }),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn disabling_two_factor_requires_the_current_password() {
    let app = spawn().await;
    let (_, token) = app.user("disable-2fa@example.com").await;
    let (_, setup) = app
        .post("/api/v1/auth/2fa/setup", Some(&token), json!({}))
        .await;
    let secret = setup["secret"].as_str().unwrap();
    app.post(
        "/api/v1/auth/2fa/enable",
        Some(&token),
        json!({ "code": current_code_for(secret) }),
    )
    .await;

    let (status, _) = app
        .post(
            "/api/v1/auth/2fa/disable",
            Some(&token),
            json!({ "current_password": "wrong-password" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = app
        .post(
            "/api/v1/auth/2fa/disable",
            Some(&token),
            json!({ "current_password": PASSWORD }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Password-only login works again.
    let (status, login) = app.login("disable-2fa@example.com", PASSWORD).await;
    assert_eq!(status, StatusCode::OK);
    assert!(login["access_token"].as_str().is_some());
}
