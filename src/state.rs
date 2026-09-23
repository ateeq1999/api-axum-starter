use std::{sync::Arc, time::Duration};

use axum::extract::FromRef;
use sqlx::PgPool;

use crate::{
    common::{
        middleware::rate_limit::RateLimiter,
        net::TrustProxy,
        security::{ApiKeyAuth, JwtSettings, SessionAuth, password},
    },
    config::Config,
    modules::{
        api_keys::{ApiKeysRepository, ApiKeysService},
        audit_log::{AuditLogRepository, AuditLogService},
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
    pub audit_log: Arc<AuditLogService>,
    /// Lets the auth extractors (in `common`) resolve API keys without depending on `api_keys`.
    pub api_key_auth: ApiKeyAuth,
    /// Lets the auth extractors (in `common`) verify sessions live against the database (role,
    /// active/deleted, token_version) without depending on `users`.
    pub session_auth: SessionAuth,
    pub jwt: Arc<JwtSettings>,
    pub rate_limiter: RateLimiter,
    pub trust_proxy: TrustProxy,
    pub config: Arc<Config>,
    pub db: PgPool,
}

impl AppState {
    pub async fn new(db: PgPool, config: Config) -> anyhow::Result<Self> {
        let mail =
            MailService::new(&config.smtp, &config.frontend)?.with_durable_outbox(db.clone());
        Self::with_mail(db, config, mail).await
    }

    /// Same wiring with an explicit mail service (tests inject an in-memory one).
    /// Fails on startup misconfiguration (bad WebAuthn relying party, unwritable upload dir).
    pub async fn with_mail(db: PgPool, config: Config, mail: MailService) -> anyhow::Result<Self> {
        password::set_max_concurrent_hashes(config.account.max_concurrent_hashes);

        let audit_log = AuditLogService::new(AuditLogRepository::new(db.clone()));
        let users = UsersService::new(
            UsersRepository::new(db.clone()),
            config.account.check_password_breaches,
            audit_log.clone(),
        );
        let auth = AuthService::new(db.clone(), users.clone(), mail.clone(), &config);
        let rate_limiter = RateLimiter::new(
            "strict",
            config.account.rate_limit_per_minute,
            Duration::from_secs(60),
            config.account.trust_proxy_headers,
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

        let session_auth = SessionAuth(Arc::new(users.clone()));

        Ok(Self {
            auth: Arc::new(auth),
            users: Arc::new(users),
            mail: Arc::new(mail),
            avatars: Arc::new(avatars),
            api_key_auth: ApiKeyAuth(api_keys.clone()),
            session_auth,
            api_keys,
            oauth: Arc::new(oauth),
            passkeys: Arc::new(passkeys),
            qr_login: Arc::new(qr_login),
            audit_log: Arc::new(audit_log),
            jwt: Arc::new(config.jwt.clone()),
            rate_limiter,
            trust_proxy: TrustProxy(config.account.trust_proxy_headers),
            config: Arc::new(config),
            db,
        })
    }
}
