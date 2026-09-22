use std::{sync::Arc, time::Duration};

use axum::extract::FromRef;
use sqlx::SqlitePool;

use crate::{
    common::{
        middleware::rate_limit::RateLimiter,
        security::{ApiKeyAuth, JwtSettings, password},
    },
    config::Config,
    modules::{
        api_keys::{ApiKeysRepository, ApiKeysService},
        auth::AuthService,
        avatars::{AvatarService, AvatarStorage},
        mail::MailService,
        oauth::{OAuthRepository, OAuthService, ProviderClient},
        passkeys::{PasskeysRepository, PasskeysService},
        qr_login::{QrLoginService, QrRepository},
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
    pub avatars: Arc<AvatarService>,
    pub api_keys: Arc<ApiKeysService>,
    pub oauth: Arc<OAuthService>,
    pub passkeys: Arc<PasskeysService>,
    pub qr_login: Arc<QrLoginService>,
    /// Lets the auth extractors (in `common`) resolve API keys without depending on `api_keys`.
    pub api_key_auth: ApiKeyAuth,
    pub jwt: Arc<JwtSettings>,
    pub rate_limiter: RateLimiter,
    pub config: Arc<Config>,
    pub db: SqlitePool,
}

impl AppState {
    pub async fn new(db: SqlitePool, config: Config) -> anyhow::Result<Self> {
        let mail = MailService::new(&config.smtp, &config.frontend)?;
        Self::with_mail(db, config, mail).await
    }

    /// Same wiring with an explicit mail service (tests inject an in-memory one).
    /// Fails on startup misconfiguration (bad WebAuthn relying party, unwritable upload dir).
    pub async fn with_mail(
        db: SqlitePool,
        config: Config,
        mail: MailService,
    ) -> anyhow::Result<Self> {
        password::set_max_concurrent_hashes(config.account.max_concurrent_hashes);

        let users = UsersService::new(UsersRepository::new(db.clone()));
        let auth = AuthService::new(db.clone(), users.clone(), mail.clone(), &config);
        let rate_limiter = RateLimiter::new(
            config.account.rate_limit_per_minute,
            Duration::from_secs(60),
        );
        let require_verified = config.account.require_verified_email;

        let avatars = AvatarService::new(
            users.clone(),
            AvatarStorage::new(config.storage.upload_dir.clone()).await?,
        );
        let api_keys = Arc::new(ApiKeysService::new(
            ApiKeysRepository::new(db.clone()),
            users.clone(),
        ));
        let oauth = OAuthService::new(
            OAuthRepository::new(db.clone()),
            users.clone(),
            ProviderClient::new(&config.oauth)?,
            config.jwt.clone(),
            require_verified,
        );
        let passkeys = PasskeysService::new(
            &config.webauthn,
            PasskeysRepository::new(db.clone()),
            users.clone(),
            config.jwt.clone(),
            require_verified,
        )?;
        let qr_login = QrLoginService::new(
            &config.qr_login,
            &config.frontend.url,
            QrRepository::new(db.clone()),
            users.clone(),
            config.jwt.clone(),
            require_verified,
        );

        Ok(Self {
            auth: Arc::new(auth),
            users: Arc::new(users),
            mail: Arc::new(mail),
            avatars: Arc::new(avatars),
            api_key_auth: ApiKeyAuth(api_keys.clone()),
            api_keys,
            oauth: Arc::new(oauth),
            passkeys: Arc::new(passkeys),
            qr_login: Arc::new(qr_login),
            jwt: Arc::new(config.jwt.clone()),
            rate_limiter,
            config: Arc::new(config),
            db,
        })
    }
}
