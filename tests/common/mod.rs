#![allow(dead_code)]

use api_starter_axum::{
    app::build_router,
    common::security::JwtSettings,
    config::{
        AccountConfig, Config, FrontendConfig, OAuthConfig, QrLoginConfig, SmtpConfig, SmtpTls,
        StorageConfig, WebauthnConfig,
    },
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
use sqlx::{Connection, PgConnection};
use tower::ServiceExt;

pub const PASSWORD: &str = "correct-horse-battery";

// ---- Postgres test database, one per test -----------------------------------------------
//
// Postgres has no ":memory:" mode, so each test gets its own throwaway database (mirroring the
// isolation SQLite's in-memory mode used to give). Creating a database per test is more overhead
// than an in-memory SQLite connection, but keeps every existing test unchanged.
//
// Cleanup: Rust's test harness has no async teardown hook, so a database is not dropped the
// moment its test finishes. Instead, the first test in *each test binary* opportunistically drops
// leftover databases from *previous* runs that are more than 10 minutes old. The age gate matters
// because `cargo test` runs multiple test binaries concurrently: an unconditional sweep could drop
// a database a sibling binary's test is using right now. In practice this means test databases
// accumulate only within a single burst of `cargo test` runs and are swept up automatically the
// next time tests run after a short idle period.

const TEST_DB_PREFIX: &str = "apistarter_test_";
const STALE_AFTER_SECS: i64 = 600;

/// Runs the stale-database sweep at most once per test binary.
static CLEANUP: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

fn admin_database_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string())
}

fn test_database_url(name: &str) -> String {
    let admin = admin_database_url();
    let base = admin
        .rsplit_once('/')
        .map_or(admin.as_str(), |(base, _)| base);
    format!("{base}/{name}")
}

async fn admin_connection() -> PgConnection {
    PgConnection::connect(&admin_database_url()).await.expect(
        "connect to the Postgres admin database (is infra-pg running? see TEST_DATABASE_URL)",
    )
}

/// Best-effort: drops leftover databases from previous runs. Never touches a database younger
/// than `STALE_AFTER_SECS`, so it cannot race a sibling test binary. Runs at most once per test
/// binary (see the `OnceCell` in `spawn_with`).
async fn cleanup_stale_test_databases() {
    let mut conn = admin_connection().await;
    let Ok(names) =
        sqlx::query_scalar::<_, String>("SELECT datname FROM pg_database WHERE datname LIKE $1")
            .bind(format!("{TEST_DB_PREFIX}%"))
            .fetch_all(&mut conn)
            .await
    else {
        return;
    };
    let now = chrono::Utc::now().timestamp();
    for name in names {
        let created_at = name
            .strip_prefix(TEST_DB_PREFIX)
            .and_then(|rest| rest.split('_').next())
            .and_then(|ts| ts.parse::<i64>().ok());
        if created_at.is_none_or(|ts| now - ts > STALE_AFTER_SECS) {
            // The name came from `pg_database` itself (and, before that, our own generator), so
            // building this statement dynamically is safe; `AssertSqlSafe` says so explicitly.
            let sql = format!(r#"DROP DATABASE IF EXISTS "{name}""#);
            let _ = sqlx::query(sqlx::AssertSqlSafe(sql))
                .execute(&mut conn)
                .await;
        }
    }
}

/// Creates a fresh, empty, uniquely named database and returns its name.
async fn create_test_database(conn: &mut PgConnection) -> String {
    let name = format!(
        "{TEST_DB_PREFIX}{}_{}",
        chrono::Utc::now().timestamp(),
        uuid::Uuid::new_v4().simple()
    );
    // `name` is generated above, never user input, so this dynamic statement is safe.
    let sql = format!(r#"CREATE DATABASE "{name}""#);
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(&mut *conn)
        .await
        .expect("create test database");
    name
}

/// A fresh, migrated database for tests that want a raw pool rather than a full `TestApp`
/// (e.g. exercising `infra::seed` directly).
pub async fn fresh_pool() -> sqlx::PgPool {
    CLEANUP.get_or_init(cleanup_stale_test_databases).await;

    let mut admin = admin_connection().await;
    let db_name = create_test_database(&mut admin).await;
    drop(admin);

    database::connect(&test_database_url(&db_name), 5)
        .await
        .unwrap()
}

pub struct TestApp {
    router: Router,
    pub state: AppState,
    pub mail: MailService,
}

pub fn test_config() -> Config {
    Config {
        // Never actually connected with: `spawn_with` below creates a fresh real database
        // per test and connects to that instead.
        database_url: String::new(),
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
        storage: StorageConfig {
            upload_dir: std::env::temp_dir()
                .join(format!("api-starter-axum-test-{}", uuid::Uuid::new_v4())),
        },
        oauth: OAuthConfig {
            public_api_url: "https://api.test".into(),
            google: None,
            github: None,
        },
        webauthn: WebauthnConfig {
            rp_id: "app.test".into(),
            rp_name: "Test".into(),
            origin: "https://app.test".into(),
        },
        qr_login: QrLoginConfig {
            ttl_secs: 120,
            poll_rate_limit_per_minute: 1000,
        },
    }
}

pub async fn spawn() -> TestApp {
    spawn_with(test_config()).await
}

pub async fn spawn_with(config: Config) -> TestApp {
    CLEANUP.get_or_init(cleanup_stale_test_databases).await;

    let mut admin = admin_connection().await;
    let db_name = create_test_database(&mut admin).await;
    drop(admin);

    let pool = database::connect(&test_database_url(&db_name), 5)
        .await
        .unwrap();
    let mail = MailService::in_memory(&config.frontend.url);
    let state = AppState::with_mail(pool, config, mail.clone())
        .await
        .unwrap();
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
        sqlx::query("UPDATE users SET role = 'admin' WHERE email = $1")
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

pub struct RawResponse {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub body: axum::body::Bytes,
}

impl RawResponse {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
}

impl TestApp {
    /// Full control over the request: arbitrary headers and a raw body.
    pub async fn raw(
        &self,
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> RawResponse {
        let mut builder = Request::builder().method(method).uri(uri);
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let response = self
            .router
            .clone()
            .oneshot(builder.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        RawResponse {
            status,
            headers,
            body,
        }
    }

    /// JSON request with extra headers (for `X-API-Key`, `X-QR-Secret`, ...).
    pub async fn json_with(
        &self,
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut all: Vec<(&str, &str)> = headers.to_vec();
        if body.is_some() {
            all.push(("content-type", "application/json"));
        }
        let bytes = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
        let response = self.raw(method, uri, &all, bytes).await;
        (response.status, response.json())
    }
}
