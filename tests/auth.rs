mod common;

use axum::http::StatusCode;
use common::{PASSWORD, spawn, spawn_with, test_config, token_from};
use serde_json::json;

#[tokio::test]
async fn register_login_and_me() {
    let app = spawn().await;
    let created = app.register("Alice@Example.com").await;
    assert_eq!(created["email"], "alice@example.com");
    assert_eq!(created["role"], "user");
    assert_eq!(created["email_verified"], false);
    assert!(created.get("password_hash").is_none());

    let token = app.login_token("alice@example.com").await;
    let (status, me) = app.get("/api/v1/auth/me", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["email"], "alice@example.com");

    let (status, _) = app.get("/api/v1/auth/me", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn duplicate_registration_and_bad_credentials() {
    let app = spawn().await;
    app.register("bob@example.com").await;

    let (status, body) = app
        .post(
            "/api/v1/auth/register",
            None,
            json!({ "email": "bob@example.com", "password": PASSWORD }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, _) = app.login("bob@example.com", "wrong-password").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app.login("nobody@example.com", PASSWORD).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = app
        .post(
            "/api/v1/auth/register",
            None,
            json!({ "email": "nope", "password": "short" }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "validation_failed");
}

#[tokio::test]
async fn registration_sends_verification_email_and_link_verifies() {
    let app = spawn().await;
    app.register("carol@example.com").await;
    let token = app.login_token("carol@example.com").await;

    let mails = app.mails_to("carol@example.com").await;
    assert_eq!(mails.len(), 1);
    assert!(
        mails[0]
            .text
            .contains("https://app.test/verify-email?token=")
    );
    let link_token = token_from(&mails[0]);

    let (status, _) = app
        .post(
            "/api/v1/auth/email/verify",
            None,
            json!({ "token": "bogus" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = app
        .post(
            "/api/v1/auth/email/verify",
            None,
            json!({ "token": link_token }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (_, me) = app.get("/api/v1/users/me", Some(&token)).await;
    assert_eq!(me["email_verified"], true);

    // single use
    let (status, _) = app
        .post(
            "/api/v1/auth/email/verify",
            None,
            json!({ "token": link_token }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // resend is a no-op once verified
    let (status, _) = app
        .post(
            "/api/v1/auth/email/verification/resend",
            Some(&token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(app.mails_to("carol@example.com").await.len(), 1);
}

#[tokio::test]
async fn resend_invalidates_the_previous_link() {
    let app = spawn().await;
    app.register("dan@example.com").await;
    let token = app.login_token("dan@example.com").await;
    let first = token_from(&app.mails_to("dan@example.com").await[0]);

    let (status, _) = app
        .post(
            "/api/v1/auth/email/verification/resend",
            Some(&token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let mails = app.mails_to("dan@example.com").await;
    assert_eq!(mails.len(), 2);
    let second = token_from(&mails[1]);
    assert_ne!(first, second);

    let (status, _) = app
        .post("/api/v1/auth/email/verify", None, json!({ "token": first }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = app
        .post(
            "/api/v1/auth/email/verify",
            None,
            json!({ "token": second }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn forgot_password_does_not_reveal_whether_the_email_exists() {
    let app = spawn().await;
    app.register("erin@example.com").await;

    let (known_status, known_body) = app
        .post(
            "/api/v1/auth/password/forgot",
            None,
            json!({ "email": "erin@example.com" }),
        )
        .await;
    let (unknown_status, unknown_body) = app
        .post(
            "/api/v1/auth/password/forgot",
            None,
            json!({ "email": "ghost@example.com" }),
        )
        .await;

    assert_eq!(known_status, StatusCode::ACCEPTED);
    assert_eq!(known_status, unknown_status);
    assert_eq!(known_body, unknown_body);

    assert!(app.mails_to("ghost@example.com").await.is_empty());
    let mails = app.mails_to("erin@example.com").await;
    // verification email from registration + reset email
    assert_eq!(mails.len(), 2);
    assert!(mails[1].text.contains("/reset-password?token="));
}

#[tokio::test]
async fn password_reset_flow() {
    let app = spawn().await;
    app.register("frank@example.com").await;
    app.post(
        "/api/v1/auth/password/forgot",
        None,
        json!({ "email": "frank@example.com" }),
    )
    .await;
    let reset_token = token_from(&app.mails_to("frank@example.com").await[1]);

    let (status, _) = app
        .post(
            "/api/v1/auth/password/reset",
            None,
            json!({ "token": reset_token, "new_password": "brand-new-password" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = app.login("frank@example.com", PASSWORD).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app.login("frank@example.com", "brand-new-password").await;
    assert_eq!(status, StatusCode::OK);

    // notice that the password changed, and the token cannot be reused
    let mails = app.mails_to("frank@example.com").await;
    assert_eq!(mails.len(), 3);
    assert_eq!(mails[2].subject, "Your password was changed");
    let (status, _) = app
        .post(
            "/api/v1/auth/password/reset",
            None,
            json!({ "token": reset_token, "new_password": "another-password-1" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // following the emailed link proves inbox control
    let token = app
        .login_token_with("frank@example.com", "brand-new-password")
        .await;
    let (_, me) = app.get("/api/v1/users/me", Some(&token)).await;
    assert_eq!(me["email_verified"], true);
}

#[tokio::test]
async fn expired_reset_token_is_rejected() {
    let app = spawn().await;
    app.register("gina@example.com").await;
    app.post(
        "/api/v1/auth/password/forgot",
        None,
        json!({ "email": "gina@example.com" }),
    )
    .await;
    let reset_token = token_from(&app.mails_to("gina@example.com").await[1]);

    sqlx::query("UPDATE auth_tokens SET expires_at = ? WHERE purpose = 'password_reset'")
        .bind(chrono::Utc::now() - chrono::Duration::minutes(1))
        .execute(&app.state.db)
        .await
        .unwrap();

    let (status, body) = app
        .post(
            "/api/v1/auth/password/reset",
            None,
            json!({ "token": reset_token, "new_password": "brand-new-password" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "bad_request");
}

#[tokio::test]
async fn reset_emails_are_limited_per_account() {
    let app = spawn().await;
    app.register("hank@example.com").await;
    for _ in 0..5 {
        let (status, _) = app
            .post(
                "/api/v1/auth/password/forgot",
                None,
                json!({ "email": "hank@example.com" }),
            )
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
    }
    // 1 verification + at most 3 resets
    assert_eq!(app.mails_to("hank@example.com").await.len(), 4);
}

#[tokio::test]
async fn change_password_requires_current_password() {
    let app = spawn().await;
    let (_, token) = app.user("ivy@example.com").await;

    let (status, _) = app
        .post(
            "/api/v1/auth/password/change",
            Some(&token),
            json!({ "current_password": "not-the-password", "new_password": "brand-new-password" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = app
        .post(
            "/api/v1/auth/password/change",
            Some(&token),
            json!({ "current_password": PASSWORD, "new_password": "brand-new-password" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        app.login("ivy@example.com", "brand-new-password").await.0,
        StatusCode::OK
    );
    assert_eq!(
        app.login("ivy@example.com", PASSWORD).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn email_change_flow() {
    let app = spawn().await;
    app.register("jack@example.com").await;
    app.register("taken@example.com").await;
    let token = app.login_token("jack@example.com").await;

    let (status, _) = app
        .post(
            "/api/v1/auth/email/change",
            Some(&token),
            json!({ "new_email": "jack.new@example.com", "current_password": "wrong-password" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = app
        .post(
            "/api/v1/auth/email/change",
            Some(&token),
            json!({ "new_email": "taken@example.com", "current_password": PASSWORD }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = app
        .post(
            "/api/v1/auth/email/change",
            Some(&token),
            json!({ "new_email": "Jack.New@example.com", "current_password": PASSWORD }),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    // nothing changes yet, and both addresses were told
    assert_eq!(
        app.login("jack@example.com", PASSWORD).await.0,
        StatusCode::OK
    );
    let new_mails = app.mails_to("jack.new@example.com").await;
    assert_eq!(new_mails.len(), 1);
    assert!(new_mails[0].text.contains("/confirm-email-change?token="));
    let old_mails = app.mails_to("jack@example.com").await;
    assert_eq!(
        old_mails.last().unwrap().subject,
        "A change of your email address was requested"
    );

    let (status, _) = app
        .post(
            "/api/v1/auth/email/change/confirm",
            None,
            json!({ "token": token_from(&new_mails[0]) }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        app.login("jack@example.com", PASSWORD).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.login("jack.new@example.com", PASSWORD).await.0,
        StatusCode::OK
    );
    let (_, me) = app.get("/api/v1/users/me", Some(&token)).await;
    assert_eq!(me["email"], "jack.new@example.com");
    assert_eq!(me["email_verified"], true);
}

#[tokio::test]
async fn login_can_require_a_verified_email() {
    let mut config = test_config();
    config.account.require_verified_email = true;
    let app = spawn_with(config).await;
    app.register("kim@example.com").await;

    let (status, body) = app.login("kim@example.com", PASSWORD).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let link_token = token_from(&app.mails_to("kim@example.com").await[0]);
    app.post(
        "/api/v1/auth/email/verify",
        None,
        json!({ "token": link_token }),
    )
    .await;
    assert_eq!(
        app.login("kim@example.com", PASSWORD).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn admin_invitation_lets_the_invitee_choose_a_password() {
    let app = spawn().await;
    let (_, admin) = app.admin("root@example.com").await;
    let (_, user) = app.user("plain@example.com").await;

    let invite = json!({ "email": "New.Hire@example.com", "display_name": "New Hire" });
    let (status, _) = app
        .post("/api/v1/auth/invitations", Some(&user), invite.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = app
        .post("/api/v1/auth/invitations", None, invite.clone())
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = app
        .post("/api/v1/auth/invitations", Some(&admin), invite)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["email"], "new.hire@example.com");

    let mails = app.mails_to("new.hire@example.com").await;
    assert_eq!(mails.len(), 1);
    assert_eq!(mails[0].subject, "You have been invited");

    let (status, _) = app
        .post(
            "/api/v1/auth/password/reset",
            None,
            json!({ "token": token_from(&mails[0]), "new_password": "my-own-password" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        app.login("new.hire@example.com", "my-own-password").await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn public_endpoints_are_rate_limited() {
    let mut config = test_config();
    config.account.rate_limit_per_minute = 3;
    let app = spawn_with(config).await;

    for _ in 0..3 {
        let (status, _) = app.login("nobody@example.com", PASSWORD).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, body) = app.login("nobody@example.com", PASSWORD).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "too_many_requests");

    // unrelated routes are unaffected
    let (status, _) = app.get("/health/live", None).await;
    assert_eq!(status, StatusCode::OK);
}
