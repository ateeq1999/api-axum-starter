mod common;

use std::collections::HashMap;

use api_starter_axum::config::OAuthProviderConfig;
use axum::{
    Form, Json, Router,
    http::{HeaderMap, Method, StatusCode},
    routing::{get, post},
};
use common::{TestApp, spawn_with, test_config};
use serde_json::{Value, json};
use url::Url;

// ---- a fake OAuth provider ------------------------------------------------------------------
//
// The "authorization code" the test passes to our callback is the profile it wants the fake
// provider to report, written `id|email|verified|name`. The token endpoint hex-encodes it into
// the access token and the profile endpoints decode it again.

fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> String {
    let bytes: Vec<u8> = (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    String::from_utf8(bytes).unwrap()
}

async fn token(Form(form): Form<HashMap<String, String>>) -> (StatusCode, Json<Value>) {
    // The client must prove it holds the PKCE verifier and identify itself.
    for required in [
        "code",
        "code_verifier",
        "client_id",
        "client_secret",
        "redirect_uri",
    ] {
        assert!(
            form.contains_key(required),
            "token request lacks {required}"
        );
    }
    if form["code"] == "denied" {
        return (
            StatusCode::OK,
            Json(json!({ "error": "bad_verification_code" })),
        );
    }
    (
        StatusCode::OK,
        Json(json!({ "access_token": hex(&form["code"]) })),
    )
}

fn profile_parts(headers: &HeaderMap) -> Vec<String> {
    let bearer = headers["authorization"]
        .to_str()
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap();
    unhex(bearer).split('|').map(str::to_string).collect()
}

async fn google_userinfo(headers: HeaderMap) -> Json<Value> {
    let p = profile_parts(&headers);
    Json(json!({ "sub": p[0], "email": p[1], "email_verified": p[2] == "true", "name": p[3] }))
}

async fn github_user(headers: HeaderMap) -> Json<Value> {
    let p = profile_parts(&headers);
    Json(
        json!({ "id": p[0].parse::<i64>().unwrap(), "login": "octo", "name": p[3], "email": null }),
    )
}

async fn github_emails(headers: HeaderMap) -> Json<Value> {
    let p = profile_parts(&headers);
    let mut emails = vec![json!({ "email": p[1], "primary": true, "verified": p[2] == "true" })];
    // A verified secondary address exists only for accounts whose primary is verified too.
    if p[2] == "true" {
        emails.insert(
            0,
            json!({ "email": "someone-else@example.com", "primary": false, "verified": true }),
        );
    }
    Json(Value::Array(emails))
}

async fn fake_provider() -> String {
    let router = Router::new()
        .route("/token", post(token))
        .route("/google/userinfo", get(google_userinfo))
        .route("/github/user", get(github_user))
        .route("/github/emails", get(github_emails));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    base
}

async fn app_with_providers() -> TestApp {
    let base = fake_provider().await;
    let mut config = test_config();
    config.oauth.google = Some(OAuthProviderConfig {
        client_id: "google-id".into(),
        client_secret: "google-secret".into(),
        authorize_url: "https://accounts.example/authorize".into(),
        token_url: format!("{base}/token"),
        userinfo_url: format!("{base}/google/userinfo"),
        emails_url: None,
    });
    config.oauth.github = Some(OAuthProviderConfig {
        client_id: "github-id".into(),
        client_secret: "github-secret".into(),
        authorize_url: "https://github.example/authorize".into(),
        token_url: format!("{base}/token"),
        userinfo_url: format!("{base}/github/user"),
        emails_url: Some(format!("{base}/github/emails")),
    });
    spawn_with(config).await
}

// ---- helpers --------------------------------------------------------------------------------

fn location(response: &common::RawResponse) -> Url {
    assert!(
        response.status.is_redirection(),
        "expected a redirect, got {}",
        response.status
    );
    Url::parse(response.headers["location"].to_str().unwrap()).unwrap()
}

fn param(url: &Url, name: &str) -> Option<String> {
    url.query_pairs()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Browser step 1: `GET /login` redirects to the provider. Returns the `state` it carries.
async fn start(app: &TestApp, provider: &str, redirect: Option<&str>) -> String {
    let uri = match redirect {
        Some(r) => format!("/api/v1/auth/oauth/{provider}/login?redirect={}", enc(r)),
        None => format!("/api/v1/auth/oauth/{provider}/login"),
    };
    let to = location(&app.raw(Method::GET, &uri, &[], vec![]).await);
    assert!(to.as_str().starts_with(&format!(
        "https://{}",
        if provider == "google" {
            "accounts.example"
        } else {
            "github.example"
        }
    )));
    assert_eq!(param(&to, "code_challenge_method").as_deref(), Some("S256"));
    assert!(param(&to, "code_challenge").is_some());
    assert_eq!(
        param(&to, "redirect_uri").unwrap(),
        format!("https://api.test/api/v1/auth/oauth/{provider}/callback")
    );
    param(&to, "state").unwrap()
}

/// Browser step 2: the provider sends the user back. Returns where our API redirects next.
async fn callback(app: &TestApp, provider: &str, state: &str, code: &str) -> Url {
    let uri = format!(
        "/api/v1/auth/oauth/{provider}/callback?code={}&state={}",
        enc(code),
        enc(state)
    );
    location(&app.raw(Method::GET, &uri, &[], vec![]).await)
}

async fn sign_in(app: &TestApp, provider: &str, profile: &str) -> Url {
    let state = start(app, provider, None).await;
    callback(app, provider, &state, profile).await
}

async fn exchange(app: &TestApp, code: &str) -> (StatusCode, Value) {
    app.post("/api/v1/auth/oauth/exchange", None, json!({ "code": code }))
        .await
}

fn assert_failed(to: &Url, reason: &str) {
    assert_eq!(to.path(), "/oauth/callback", "{to}");
    assert_eq!(param(to, "error").as_deref(), Some(reason), "{to}");
    assert!(param(to, "code").is_none());
}

// ---- tests ----------------------------------------------------------------------------------

#[tokio::test]
async fn lists_only_configured_providers() {
    let plain = common::spawn().await;
    let (status, body) = plain.get("/api/v1/auth/oauth/providers", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
    let response = plain
        .raw(Method::GET, "/api/v1/auth/oauth/google/login", &[], vec![])
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);

    let app = app_with_providers().await;
    let (_, body) = app.get("/api/v1/auth/oauth/providers", None).await;
    assert_eq!(body[0]["provider"], "google");
    assert_eq!(body[1]["provider"], "github");
    assert_eq!(body[1]["login_url"], "/api/v1/auth/oauth/github/login");
    let unknown = app
        .raw(Method::GET, "/api/v1/auth/oauth/myspace/login", &[], vec![])
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn first_sign_in_creates_a_verified_passwordless_account() {
    let app = app_with_providers().await;

    let state = start(&app, "google", Some("/settings/security")).await;
    let to = callback(
        &app,
        "google",
        &state,
        "g-1|New.Person@Example.com|true|New Person",
    )
    .await;
    assert_eq!(to.path(), "/oauth/callback");
    assert_eq!(to.host_str(), Some("app.test"));
    assert_eq!(
        param(&to, "redirect").as_deref(),
        Some("/settings/security")
    );
    let code = param(&to, "code").expect("a one-time code");

    let (status, token) = exchange(&app, &code).await;
    assert_eq!(status, StatusCode::OK, "{token}");
    assert_eq!(token["token_type"], "Bearer");
    let access = token["access_token"].as_str().unwrap();

    let (_, me) = app.get("/api/v1/users/me", Some(access)).await;
    assert_eq!(me["email"], "new.person@example.com");
    assert_eq!(me["display_name"], "New Person");
    assert_eq!(me["email_verified"], true);
    assert_eq!(me["has_password"], false);
    assert_eq!(me["role"], "user");

    // the code is single use
    let (status, body) = exchange(&app, &code).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(exchange(&app, "made-up").await.0, StatusCode::BAD_REQUEST);

    // no password can ever match an OAuth-only account
    let (status, _) = app.login("new.person@example.com", "anything-at-all").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn signing_in_again_reuses_the_same_account() {
    let app = app_with_providers().await;
    let profile = "g-2|repeat@example.com|true|Repeat";

    let first = param(&sign_in(&app, "google", profile).await, "code").unwrap();
    let second = param(&sign_in(&app, "google", profile).await, "code").unwrap();
    let id = |code: String| {
        let app = &app;
        async move {
            let token = exchange(app, &code).await.1["access_token"]
                .as_str()
                .unwrap()
                .to_string();
            app.get("/api/v1/users/me", Some(&token)).await.1["id"].clone()
        }
    };
    assert_eq!(id(first).await, id(second).await);

    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    let identities: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_identities")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!((users, identities), (1, 1));
}

#[tokio::test]
async fn a_verified_local_account_with_the_same_email_is_linked() {
    let app = app_with_providers().await;
    let (user_id, _) = app.user("local@example.com").await;
    sqlx::query("UPDATE users SET email_verified_at = $1 WHERE email = 'local@example.com'")
        .bind(chrono::Utc::now())
        .execute(&app.state.db)
        .await
        .unwrap();

    let to = sign_in(&app, "github", "77|local@example.com|true|Local Person").await;
    let (_, token) = exchange(&app, &param(&to, "code").unwrap()).await;
    let (_, me) = app
        .get(
            "/api/v1/users/me",
            Some(token["access_token"].as_str().unwrap()),
        )
        .await;
    assert_eq!(me["id"], user_id.as_str());
    assert_eq!(me["has_password"], true, "the password keeps working");
    assert_eq!(
        app.login("local@example.com", common::PASSWORD).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn an_unverified_local_account_is_never_taken_over() {
    let app = app_with_providers().await;
    // Someone registered this address without proving they own it.
    app.register("victim@example.com").await;

    let to = sign_in(&app, "google", "g-3|victim@example.com|true|Real Owner").await;
    assert_failed(&to, "account_exists_unverified");
    let identities: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_identities")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(identities, 0);
}

#[tokio::test]
async fn unverified_or_missing_provider_emails_are_refused() {
    let app = app_with_providers().await;
    assert_failed(
        &sign_in(&app, "google", "g-4|a@example.com|false|A").await,
        "email_not_verified",
    );
    assert_failed(
        &sign_in(&app, "github", "88|b@example.com|false|B").await,
        "email_not_verified",
    );
    assert_failed(
        &sign_in(&app, "google", "g-5||false|C").await,
        "email_not_verified",
    );
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(users, 0);
}

#[tokio::test]
async fn github_uses_its_verified_primary_email() {
    let app = app_with_providers().await;
    let to = sign_in(&app, "github", "99|gh.user@example.com|true|Gh User").await;
    let (_, token) = exchange(&app, &param(&to, "code").unwrap()).await;
    let (_, me) = app
        .get(
            "/api/v1/users/me",
            Some(token["access_token"].as_str().unwrap()),
        )
        .await;
    assert_eq!(me["email"], "gh.user@example.com");
}

#[tokio::test]
async fn state_protects_against_forged_replayed_and_swapped_callbacks() {
    let app = app_with_providers().await;
    let profile = "g-6|state@example.com|true|State";

    // no state, made-up state
    let no_state = location(
        &app.raw(
            Method::GET,
            "/api/v1/auth/oauth/google/callback?code=x",
            &[],
            vec![],
        )
        .await,
    );
    assert_failed(&no_state, "invalid_state");
    assert_failed(
        &callback(&app, "google", "forged", profile).await,
        "invalid_state",
    );

    // a state is single use
    let state = start(&app, "google", None).await;
    assert!(param(&callback(&app, "google", &state, profile).await, "code").is_some());
    assert_failed(
        &callback(&app, "google", &state, profile).await,
        "invalid_state",
    );

    // a state started for Google cannot complete a GitHub callback
    let google_state = start(&app, "google", None).await;
    assert_failed(
        &callback(&app, "github", &google_state, "1|a@example.com|true|A").await,
        "invalid_state",
    );

    // expired
    let expired = start(&app, "google", None).await;
    sqlx::query("UPDATE oauth_states SET expires_at = $1")
        .bind(chrono::Utc::now() - chrono::Duration::minutes(1))
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_failed(
        &callback(&app, "google", &expired, profile).await,
        "invalid_state",
    );
}

#[tokio::test]
async fn provider_errors_and_cancellations_land_on_the_frontend() {
    let app = app_with_providers().await;

    let state = start(&app, "google", None).await;
    let uri = format!(
        "/api/v1/auth/oauth/google/callback?error=access_denied&state={}",
        enc(&state)
    );
    assert_failed(
        &location(&app.raw(Method::GET, &uri, &[], vec![]).await),
        "access_denied",
    );

    let state = start(&app, "google", None).await;
    assert_failed(
        &callback(&app, "google", &state, "denied").await,
        "provider_error",
    );
}

#[tokio::test]
async fn redirect_targets_cannot_leave_the_frontend() {
    let app = app_with_providers().await;
    for evil in [
        "https://evil.example/steal",
        "//evil.example",
        "javascript:alert(1)",
    ] {
        let state = start(&app, "google", Some(evil)).await;
        let to = callback(&app, "google", &state, "g-7|redir@example.com|true|R").await;
        assert_eq!(to.host_str(), Some("app.test"), "{evil}");
        assert_eq!(param(&to, "redirect").as_deref(), Some("/"), "{evil}");
    }
}

#[tokio::test]
async fn a_deactivated_account_cannot_sign_in() {
    let app = app_with_providers().await;
    let profile = "g-8|off@example.com|true|Off";
    param(&sign_in(&app, "google", profile).await, "code").unwrap();
    sqlx::query("UPDATE users SET is_active = FALSE WHERE email = 'off@example.com'")
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_failed(&sign_in(&app, "google", profile).await, "account_disabled");
}

#[tokio::test]
async fn linking_and_unlinking_from_a_signed_in_account() {
    let app = app_with_providers().await;
    let (_, token) = app.user("linker@example.com").await;

    // start linking: the SPA gets the URL as JSON (a browser navigation cannot send a token)
    let (status, body) = app
        .post("/api/v1/auth/oauth/github/link", Some(&token), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let authorize = Url::parse(body["authorize_url"].as_str().unwrap()).unwrap();
    let state = param(&authorize, "state").unwrap();

    let to = callback(
        &app,
        "github",
        &state,
        "55|whatever@example.com|true|Linker",
    )
    .await;
    assert_eq!(param(&to, "linked").as_deref(), Some("github"));
    assert_eq!(param(&to, "redirect").as_deref(), Some("/settings"));
    assert!(
        param(&to, "code").is_none(),
        "linking must not sign anyone in"
    );

    let (_, identities) = app.get("/api/v1/auth/oauth/identities", Some(&token)).await;
    assert_eq!(identities[0]["provider"], "github");
    assert_eq!(identities[0]["email"], "whatever@example.com");

    // the same GitHub account cannot be attached to a second user
    let (_, other) = app.user("other@example.com").await;
    let body = app
        .post("/api/v1/auth/oauth/github/link", Some(&other), json!({}))
        .await
        .1;
    let state = param(
        &Url::parse(body["authorize_url"].as_str().unwrap()).unwrap(),
        "state",
    )
    .unwrap();
    assert_failed(
        &callback(
            &app,
            "github",
            &state,
            "55|whatever@example.com|true|Linker",
        )
        .await,
        "already_linked",
    );

    // signing in with GitHub now reaches the linked account
    let to = sign_in(&app, "github", "55|whatever@example.com|true|Linker").await;
    let session = exchange(&app, &param(&to, "code").unwrap()).await.1;
    let (_, me) = app
        .get(
            "/api/v1/users/me",
            Some(session["access_token"].as_str().unwrap()),
        )
        .await;
    assert_eq!(me["email"], "linker@example.com");

    // unlink (allowed: the account still has its password)
    let (status, _) = app.delete("/api/v1/auth/oauth/github", Some(&token)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.delete("/api/v1/auth/oauth/github", Some(&token))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, identities) = app.get("/api/v1/auth/oauth/identities", Some(&token)).await;
    assert_eq!(identities, json!([]));
}

#[tokio::test]
async fn the_last_sign_in_method_cannot_be_removed() {
    let app = app_with_providers().await;
    let to = sign_in(&app, "google", "g-9|only@example.com|true|Only").await;
    let token = exchange(&app, &param(&to, "code").unwrap()).await.1["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let (status, body) = app.delete("/api/v1/auth/oauth/google", Some(&token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // once a second way in exists (a password via the reset flow) it can go
    app.post(
        "/api/v1/auth/password/forgot",
        None,
        json!({ "email": "only@example.com" }),
    )
    .await;
    let reset = common::token_from(&app.mails_to("only@example.com").await[0]);
    let (status, body) = app
        .post(
            "/api/v1/auth/password/reset",
            None,
            json!({ "token": reset, "new_password": "my-new-password-1" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Resetting the password bumps token_version, so the pre-reset token is no longer valid
    // (the whole point: an attacker who stole it loses access the moment the owner resets).
    let token = app
        .login_token_with("only@example.com", "my-new-password-1")
        .await;
    let (_, me) = app.get("/api/v1/users/me", Some(&token)).await;
    assert_eq!(me["has_password"], true);
    assert_eq!(
        app.delete("/api/v1/auth/oauth/google", Some(&token))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn managing_links_requires_an_interactive_session() {
    let app = app_with_providers().await;
    let (_, session) = app.user("keyed@example.com").await;
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

    assert_eq!(
        app.post("/api/v1/auth/oauth/github/link", Some(&key), json!({}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.delete("/api/v1/auth/oauth/github", Some(&key)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.post("/api/v1/auth/oauth/github/link", None, json!({}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
