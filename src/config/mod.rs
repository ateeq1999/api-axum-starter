use std::{env, fmt, net::SocketAddr, str::FromStr};

pub mod features;

pub use features::{
    MetricsConfig, OAuthConfig, OAuthProviderConfig, QrLoginConfig, StorageConfig, WebauthnConfig,
};

use crate::common::security::{JwtSettings, password};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable `{0}`")]
    Missing(&'static str),
    #[error("invalid value for `{name}`: {reason}")]
    Invalid { name: &'static str, reason: String },
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub jwt: JwtSettings,
    pub smtp: SmtpConfig,
    pub frontend: FrontendConfig,
    pub account: AccountConfig,
    pub bootstrap_admin: Option<BootstrapAdmin>,
    pub storage: StorageConfig,
    pub oauth: OAuthConfig,
    pub webauthn: WebauthnConfig,
    pub qr_login: QrLoginConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpTls {
    None,
    StartTls,
    Tls,
}

impl FromStr for SmtpTls {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "starttls" => Ok(Self::StartTls),
            "tls" => Ok(Self::Tls),
            other => Err(format!("`{other}` is not one of none | starttls | tls")),
        }
    }
}

#[derive(Clone)]
pub struct SmtpConfig {
    /// `false` selects the logging transport: nothing leaves the process.
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub tls: SmtpTls,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from: String,
}

impl fmt::Debug for SmtpConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmtpConfig")
            .field("enabled", &self.enabled)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("tls", &self.tls)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("from", &self.from)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct FrontendConfig {
    /// Base URL used to build links inside emails (no trailing slash).
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct AccountConfig {
    pub password_reset_ttl_minutes: i64,
    pub email_verification_ttl_hours: i64,
    pub email_change_ttl_minutes: i64,
    pub require_verified_email: bool,
    pub rate_limit_per_minute: u32,
    /// Cap on simultaneous argon2 operations (each allocates about 19 MiB).
    pub max_concurrent_hashes: usize,
    /// Trust `X-Forwarded-For` (its right-most entry) for the client IP used by rate limiting
    /// and QR-login's "requested from" display, instead of the TCP peer address. Only turn this
    /// on when this app is deployed behind exactly one trusted reverse proxy that sets/appends
    /// that header itself — otherwise a client can spoof its own IP.
    pub trust_proxy_headers: bool,
}

#[derive(Clone)]
pub struct BootstrapAdmin {
    pub email: String,
    pub password: String,
}

impl fmt::Debug for BootstrapAdmin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BootstrapAdmin")
            .field("email", &self.email)
            .field("password", &"<redacted>")
            .finish()
    }
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError::Missing(name))
}

pub(crate) fn non_empty(name: &'static str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

pub(crate) fn optional<T>(name: &'static str, default: T) -> Result<T, ConfigError>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    match env::var(name) {
        Ok(raw) => raw.parse().map_err(|e: T::Err| ConfigError::Invalid {
            name,
            reason: e.to_string(),
        }),
        Err(_) => Ok(default),
    }
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let jwt_secret = required("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            return Err(ConfigError::Invalid {
                name: "JWT_SECRET",
                reason: "must be at least 32 characters".into(),
            });
        }

        let max_concurrent_hashes: usize = optional(
            "MAX_CONCURRENT_HASHES",
            password::DEFAULT_MAX_CONCURRENT_HASHES,
        )?;
        if max_concurrent_hashes == 0 {
            return Err(ConfigError::Invalid {
                name: "MAX_CONCURRENT_HASHES",
                reason: "must be at least 1".into(),
            });
        }

        let smtp_enabled: bool = optional("MAIL_ENABLED", false)?;
        let smtp = if smtp_enabled {
            SmtpConfig {
                enabled: true,
                host: required("SMTP_HOST")?,
                port: optional("SMTP_PORT", 587)?,
                tls: optional("SMTP_TLS", SmtpTls::StartTls)?,
                username: non_empty("SMTP_USERNAME"),
                password: non_empty("SMTP_PASSWORD"),
                from: required("MAIL_FROM")?,
            }
        } else {
            SmtpConfig {
                enabled: false,
                host: String::new(),
                port: 0,
                tls: SmtpTls::None,
                username: None,
                password: None,
                from: optional("MAIL_FROM", "App <no-reply@localhost>".to_string())?,
            }
        };

        let bootstrap_admin = match (non_empty("ADMIN_EMAIL"), non_empty("ADMIN_PASSWORD")) {
            (Some(email), Some(password)) => Some(BootstrapAdmin { email, password }),
            (None, None) => None,
            _ => {
                return Err(ConfigError::Invalid {
                    name: "ADMIN_EMAIL",
                    reason: "ADMIN_EMAIL and ADMIN_PASSWORD must be set together".into(),
                });
            }
        };

        let bind_addr: SocketAddr = optional("BIND_ADDR", "127.0.0.1:3000".parse().unwrap())?;
        let frontend = FrontendConfig {
            url: optional("FRONTEND_URL", "http://localhost:5173".to_string())?
                .trim_end_matches('/')
                .to_string(),
        };
        let webauthn = WebauthnConfig::from_env(&frontend.url)?;

        Ok(Self {
            database_url: required("DATABASE_URL")?,
            bind_addr,
            jwt: JwtSettings {
                secret: jwt_secret,
                ttl_secs: optional("JWT_TTL_SECS", 3600)?,
            },
            smtp,
            frontend,
            account: AccountConfig {
                password_reset_ttl_minutes: optional("PASSWORD_RESET_TTL_MINUTES", 30)?,
                email_verification_ttl_hours: optional("EMAIL_VERIFICATION_TTL_HOURS", 24)?,
                email_change_ttl_minutes: optional("EMAIL_CHANGE_TTL_MINUTES", 60)?,
                require_verified_email: optional("REQUIRE_VERIFIED_EMAIL", false)?,
                rate_limit_per_minute: optional("RATE_LIMIT_PER_MINUTE", 20)?,
                max_concurrent_hashes,
                trust_proxy_headers: optional("TRUST_PROXY_HEADERS", false)?,
            },
            bootstrap_admin,
            storage: StorageConfig::from_env()?,
            oauth: OAuthConfig::from_env(bind_addr)?,
            webauthn,
            qr_login: QrLoginConfig::from_env()?,
            metrics: MetricsConfig::from_env()?,
        })
    }
}
