use std::{sync::Arc, time::Duration};

use axum::extract::FromRef;
use sqlx::SqlitePool;

use crate::{
    common::{
        middleware::rate_limit::RateLimiter,
        security::{JwtSettings, password},
    },
    config::Config,
    modules::{
        auth::AuthService,
        mail::{MailError, MailService},
        users::{UsersRepository, UsersService},
    },
};

/// Shared application state. axum clones it for every request, so every field is a cheap handle
/// (an `Arc`, or a type that is an `Arc` inside): cloning is a few refcount increments and never
/// allocates. Keep it that way: a `String` or `Vec` field here would be copied on every request.
#[derive(Clone, FromRef)]
pub struct AppState {
    pub auth: Arc<AuthService>,
    pub users: Arc<UsersService>,
    pub mail: Arc<MailService>,
    pub jwt: Arc<JwtSettings>,
    pub rate_limiter: RateLimiter,
    pub config: Arc<Config>,
    pub db: SqlitePool,
}

impl AppState {
    pub fn new(db: SqlitePool, config: Config) -> Result<Self, MailError> {
        let mail = MailService::new(&config.smtp, &config.frontend)?;
        Ok(Self::with_mail(db, config, mail))
    }

    /// Same wiring with an explicit mail service (tests inject an in-memory one).
    pub fn with_mail(db: SqlitePool, config: Config, mail: MailService) -> Self {
        password::set_max_concurrent_hashes(config.account.max_concurrent_hashes);

        let users = UsersService::new(UsersRepository::new(db.clone()));
        let auth = AuthService::new(db.clone(), users.clone(), mail.clone(), &config);
        let rate_limiter = RateLimiter::new(
            config.account.rate_limit_per_minute,
            Duration::from_secs(60),
        );
        Self {
            auth: Arc::new(auth),
            users: Arc::new(users),
            mail: Arc::new(mail),
            jwt: Arc::new(config.jwt.clone()),
            rate_limiter,
            config: Arc::new(config),
            db,
        }
    }
}
