#![allow(dead_code)]

use api_starter_axum::{
    app::build_router,
    common::security::JwtSettings,
    config::{AccountConfig, Config, FrontendConfig, SmtpConfig, SmtpTls},
    infra::database,
    modules::mail::{MailService, OutgoingMail},
    state::AppState,
};
use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

pub const PASSWORD: &str = "correct-horse-battery";

pub struct TestApp {
    router: Router,
    pub state: AppState,
    pub mail: MailService,
}

pub fn test_config() -> Config {
    Config {
        database_url: "sqlite::memory:".into(),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        jwt: JwtSettings {
            secret: "0123456789abcdef0123456789abcdef".into(),
            ttl_secs: 3600,
        },
        smtp: SmtpConfig {
            enabled: false,
            host: String::new(),
            port: 0,
            tls: SmtpTls::None,
            username: None,
            password: None,
            from: "App <no-reply@example.com>".into(),
        },
        frontend: FrontendConfig {
            url: "https://app.test".into(),
        },
        account: AccountConfig {
            password_reset_ttl_minutes: 30,
            email_verification_ttl_hours: 24,
            email_change_ttl_minutes: 60,
            require_verified_email: false,
            rate_limit_per_minute: 1000,
            max_concurrent_hashes: 8,
        },
        bootstrap_admin: None,
    }
}

pub async fn spawn() -> TestApp {
    spawn_with(test_config()).await
}

pub async fn spawn_with(config: Config) -> TestApp {
    // One connection: every in-memory SQLite connection is its own database.
    let pool = database::connect("sqlite::memory:", 1).await.unwrap();
    let mail = MailService::in_memory(&config.frontend.url);
    let state = AppState::with_mail(pool, config, mail.clone());
    TestApp {
        router: build_router(state.clone()),
        state,
        mail,
    }
}

impl TestApp {
    pub async fn request(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let body = match body {
            Some(json) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(json.to_string())
            }
            None => Body::empty(),
        };
        let response = self
            .router
            .clone()
            .oneshot(builder.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
    }

    pub async fn post(&self, uri: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        self.request(Method::POST, uri, token, Some(body)).await
    }

    pub async fn get(&self, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        self.request(Method::GET, uri, token, None).await
    }

    pub async fn patch(&self, uri: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        self.request(Method::PATCH, uri, token, Some(body)).await
    }

    pub async fn delete(&self, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        self.request(Method::DELETE, uri, token, None).await
    }

    pub async fn register(&self, email: &str) -> Value {
        let (status, body) = self
            .post(
                "/api/v1/auth/register",
                None,
                serde_json::json!({ "email": email, "password": PASSWORD }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "register failed: {body}");
        body
    }

    pub async fn login(&self, email: &str, password: &str) -> (StatusCode, Value) {
        self.post(
            "/api/v1/auth/login",
            None,
            serde_json::json!({ "email": email, "password": password }),
        )
        .await
    }

    pub async fn login_token(&self, email: &str) -> String {
        self.login_token_with(email, PASSWORD).await
    }

    pub async fn login_token_with(&self, email: &str, password: &str) -> String {
        let (status, body) = self.login(email, password).await;
        assert_eq!(status, StatusCode::OK, "login failed: {body}");
        body["access_token"].as_str().unwrap().to_string()
    }

    /// Registers a normal user and returns `(id, token)`.
    pub async fn user(&self, email: &str) -> (String, String) {
        let created = self.register(email).await;
        (
            created["id"].as_str().unwrap().to_string(),
            self.login_token(email).await,
        )
    }

    /// Registers a user, promotes them in the database, and returns `(id, token)`.
    pub async fn admin(&self, email: &str) -> (String, String) {
        let created = self.register(email).await;
        sqlx::query("UPDATE users SET role = 'admin' WHERE email = ?")
            .bind(email)
            .execute(&self.state.db)
            .await
            .unwrap();
        (
            created["id"].as_str().unwrap().to_string(),
            self.login_token(email).await,
        )
    }

    /// Emails sent to `to`, oldest first.
    pub async fn mails_to(&self, to: &str) -> Vec<OutgoingMail> {
        self.mail
            .sent()
            .await
            .into_iter()
            .filter(|m| m.to == to)
            .collect()
    }
}

/// Pulls the `token=` value out of the first link in a mail body.
pub fn token_from(mail: &OutgoingMail) -> String {
    let start = mail.text.find("token=").expect("mail has no token link") + "token=".len();
    mail.text[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}
