mod common;

use api_starter_axum::common::security::{Role, jwt};
use axum::http::StatusCode;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use common::{TestApp, spawn};
use serde_json::{Value, json};
use webauthn_authenticator_rs::{WebauthnAuthenticator, softpasskey::SoftPasskey};
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse, Url};

const ORIGIN: &str = "https://app.test";

type Authenticator = WebauthnAuthenticator<SoftPasskey>;

fn authenticator() -> Authenticator {
    WebauthnAuthenticator::new(SoftPasskey::new(true))
}

fn origin(s: &str) -> Url {
    Url::parse(s).unwrap()
}

/// Runs the whole registration ceremony with a software authenticator.
/// Returns the API's answer and the credential the "browser" produced.
async fn register(
    app: &TestApp,
    token: &str,
    wa: &mut Authenticator,
    name: &str,
    at: &str,
) -> (StatusCode, Value, Value) {
    let (status, begin) = app
        .post(
            "/api/v1/auth/passkeys/register/begin",
            Some(token),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{begin}");
    let options: CreationChallengeResponse =
        serde_json::from_value(begin["options"].clone()).unwrap();
    let credential = wa
        .do_registration(origin(at), options)
        .expect("authenticator registers");
    let credential = serde_json::to_value(&credential).unwrap();

    let (status, body) = app
        .post(
            "/api/v1/auth/passkeys/register/finish",
            Some(token),
            json!({ "challenge_id": begin["challenge_id"], "name": name, "credential": credential }),
        )
        .await;
    (status, body, credential)
}

/// The browser's half of a usernameless sign-in, as a real discoverable authenticator behaves:
/// it finds its credential without an allow-list and reports the user handle it stored.
fn answer_login(
    wa: &mut Authenticator,
    begin: &Value,
    credential_id: &Value,
    user_id: &str,
    at: &str,
) -> Value {
    let mut options = begin["options"].clone();
    // the software authenticator needs an allow-list to find its key; real ones do not
    options["publicKey"]["allowCredentials"] =
        json!([{ "type": "public-key", "id": credential_id }]);
    let options: RequestChallengeResponse = serde_json::from_value(options).unwrap();
    let credential = wa
        .do_authentication(origin(at), options)
        .expect("authenticator signs");

    let mut credential = serde_json::to_value(&credential).unwrap();
    let handle = uuid::Uuid::parse_str(user_id).unwrap();
    credential["response"]["userHandle"] = json!(URL_SAFE_NO_PAD.encode(handle.as_bytes()));
    credential
}

async fn login_with_passkey(
    app: &TestApp,
    wa: &mut Authenticator,
    credential_id: &Value,
    user_id: &str,
) -> (StatusCode, Value) {
    let (status, begin) = app
        .post("/api/v1/auth/passkeys/login/begin", None, json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{begin}");
    let credential = answer_login(wa, &begin, credential_id, user_id, ORIGIN);
    app.post(
        "/api/v1/auth/passkeys/login/finish",
        None,
        json!({ "challenge_id": begin["challenge_id"], "credential": credential }),
    )
    .await
}

#[tokio::test]
async fn register_list_and_remove_a_passkey() {
    let app = spawn().await;
    let (_, token) = app.user("pk@example.com").await;
    let mut wa = authenticator();

    let (status, created, _) = register(&app, &token, &mut wa, "MacBook Touch ID", ORIGIN).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["name"], "MacBook Touch ID");
    assert!(created["last_used_at"].is_null());
    assert!(
        created.get("credential_json").is_none(),
        "key material is never returned"
    );

    let (_, list) = app.get("/api/v1/auth/passkeys", Some(&token)).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["id"], created["id"]);

    // the account keeps its password, so removing the passkey is allowed
    let path = format!("/api/v1/auth/passkeys/{}", created["id"].as_str().unwrap());
    assert_eq!(
        app.delete(&path, Some(&token)).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.delete(&path, Some(&token)).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, list) = app.get("/api/v1/auth/passkeys", Some(&token)).await;
    assert_eq!(list, json!([]));
}

#[tokio::test]
async fn sign_in_with_a_passkey() {
    let app = spawn().await;
    let (user_id, token) = app.user("login@example.com").await;
    let mut wa = authenticator();
    let (_, _, reg) = register(&app, &token, &mut wa, "Laptop", ORIGIN).await;
    let credential_id = reg["rawId"].clone();

    let (status, session) = login_with_passkey(&app, &mut wa, &credential_id, &user_id).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_eq!(session["token_type"], "Bearer");
    let (_, me) = app
        .get(
            "/api/v1/users/me",
            Some(session["access_token"].as_str().unwrap()),
        )
        .await;
    assert_eq!(me["email"], "login@example.com");

    // last_used_at is recorded, and signing in again works (the signature counter moves on)
    let (_, list) = app.get("/api/v1/auth/passkeys", Some(&token)).await;
    assert!(list[0]["last_used_at"].is_string());
    assert_eq!(
        login_with_passkey(&app, &mut wa, &credential_id, &user_id)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_challenge_can_be_answered_only_once() {
    let app = spawn().await;
    let (user_id, token) = app.user("replay@example.com").await;
    let mut wa = authenticator();
    let (_, _, reg) = register(&app, &token, &mut wa, "Key", ORIGIN).await;
    let credential_id = reg["rawId"].clone();

    let (_, begin) = app
        .post("/api/v1/auth/passkeys/login/begin", None, json!({}))
        .await;
    let credential = answer_login(&mut wa, &begin, &credential_id, &user_id, ORIGIN);
    let finish = json!({ "challenge_id": begin["challenge_id"], "credential": credential });
    assert_eq!(
        app.post("/api/v1/auth/passkeys/login/finish", None, finish.clone())
            .await
            .0,
        StatusCode::OK
    );
    // an attacker replaying the captured response gets nowhere
    let (status, body) = app
        .post("/api/v1/auth/passkeys/login/finish", None, finish)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error"]["message"], "passkey sign-in failed");
}

#[tokio::test]
async fn responses_from_the_wrong_site_are_rejected() {
    let app = spawn().await;
    let (user_id, token) = app.user("phish@example.com").await;
    let mut wa = authenticator();

    // A sibling subdomain (say a hijacked `evil.app.test`) is allowed by the browser to use the
    // RP id `app.test`, so only the server's exact-origin check stops it.
    let (status, body, _) =
        register(&app, &token, &mut wa, "Phished", "https://evil.app.test").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // a legitimate credential, but the assertion is produced for another origin
    let (_, _, reg) = register(&app, &token, &mut wa, "Real", ORIGIN).await;
    let credential_id = reg["rawId"].clone();
    let (_, begin) = app
        .post("/api/v1/auth/passkeys/login/begin", None, json!({}))
        .await;
    let credential = answer_login(
        &mut wa,
        &begin,
        &credential_id,
        &user_id,
        "https://evil.app.test",
    );
    let (status, _) = app
        .post(
            "/api/v1/auth/passkeys/login/finish",
            None,
            json!({ "challenge_id": begin["challenge_id"], "credential": credential }),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unknown_passkeys_and_expired_challenges_fail() {
    let app = spawn().await;
    let (user_id, token) = app.user("gone@example.com").await;
    let mut wa = authenticator();
    let (_, created, reg) = register(&app, &token, &mut wa, "Key", ORIGIN).await;
    let credential_id = reg["rawId"].clone();

    // expired challenge
    let (_, begin) = app
        .post("/api/v1/auth/passkeys/login/begin", None, json!({}))
        .await;
    let credential = answer_login(&mut wa, &begin, &credential_id, &user_id, ORIGIN);
    sqlx::query("UPDATE webauthn_challenges SET expires_at = ?")
        .bind(chrono::Utc::now() - chrono::Duration::minutes(1))
        .execute(&app.state.db)
        .await
        .unwrap();
    let (status, _) = app
        .post(
            "/api/v1/auth/passkeys/login/finish",
            None,
            json!({ "challenge_id": begin["challenge_id"], "credential": credential }),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // a passkey the server no longer knows about
    let path = format!("/api/v1/auth/passkeys/{}", created["id"].as_str().unwrap());
    assert_eq!(
        app.delete(&path, Some(&token)).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        login_with_passkey(&app, &mut wa, &credential_id, &user_id)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // a made-up challenge id
    let (status, _) = app
        .post(
            "/api/v1/auth/passkeys/login/finish",
            None,
            json!({ "challenge_id": "nope", "credential": {} }),
        )
        .await;
    assert!(status.is_client_error());
}

#[tokio::test]
async fn deactivated_accounts_cannot_use_their_passkeys() {
    let app = spawn().await;
    let (user_id, token) = app.user("off@example.com").await;
    let mut wa = authenticator();
    let (_, _, reg) = register(&app, &token, &mut wa, "Key", ORIGIN).await;
    let credential_id = reg["rawId"].clone();

    sqlx::query("UPDATE users SET is_active = 0")
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_eq!(
        login_with_passkey(&app, &mut wa, &credential_id, &user_id)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn the_last_sign_in_method_cannot_be_removed() {
    let app = spawn().await;
    // an account created through OAuth has no password
    let user = app
        .state
        .users
        .create_passwordless("nopass@example.com", None, true)
        .await
        .unwrap();
    let token = jwt::issue(user.id, Role::User, &app.state.jwt.secret, 3600).unwrap();

    let mut wa = authenticator();
    let (status, created, _) = register(&app, &token, &mut wa, "Only way in", ORIGIN).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let path = format!("/api/v1/auth/passkeys/{}", created["id"].as_str().unwrap());
    let (status, body) = app.delete(&path, Some(&token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (_, list) = app.get("/api/v1/auth/passkeys", Some(&token)).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn managing_passkeys_needs_an_interactive_session() {
    let app = spawn().await;
    let (_, session) = app.user("guard@example.com").await;
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

    for uri in ["/api/v1/auth/passkeys/register/begin"] {
        assert_eq!(
            app.post(uri, None, json!({})).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.post(uri, Some(&key), json!({})).await.0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        app.get("/api/v1/auth/passkeys", Some(&key)).await.0,
        StatusCode::FORBIDDEN
    );
    // signing in is public
    assert_eq!(
        app.post("/api/v1/auth/passkeys/login/begin", None, json!({}))
            .await
            .0,
        StatusCode::OK
    );
}
