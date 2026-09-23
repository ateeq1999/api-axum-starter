//! Configuration of the optional feature modules: uploads, OAuth, WebAuthn, QR login.

use std::{fmt, net::SocketAddr, path::PathBuf};

use super::{ConfigError, non_empty, optional};

#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Where profile photos are stored (created at startup).
    pub upload_dir: PathBuf,
}

impl StorageConfig {
    pub(super) fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            upload_dir: optional("UPLOAD_DIR", PathBuf::from("./uploads"))?,
        })
    }
}

/// One OAuth provider. The endpoint URLs default to the real ones and are only overridden in
/// tests, which point them at a fake provider.
#[derive(Clone)]
pub struct OAuthProviderConfig {
    pub client_id: String,
    pub client_secret: String,
    pub authorize_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    /// GitHub only: lists the account's email addresses (the profile's email may be private).
    pub emails_url: Option<String>,
}

impl fmt::Debug for OAuthProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthProviderConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("authorize_url", &self.authorize_url)
            .finish()
    }
}

impl OAuthProviderConfig {
    pub fn google(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            userinfo_url: "https://openidconnect.googleapis.com/v1/userinfo".into(),
            emails_url: None,
        }
    }

    pub fn github(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            authorize_url: "https://github.com/login/oauth/authorize".into(),
            token_url: "https://github.com/login/oauth/access_token".into(),
            userinfo_url: "https://api.github.com/user".into(),
            emails_url: Some("https://api.github.com/user/emails".into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// Public base URL of this API. The redirect URI registered at each provider is
    /// `{public_api_url}/api/v1/auth/oauth/<provider>/callback`.
    pub public_api_url: String,
    pub google: Option<OAuthProviderConfig>,
    pub github: Option<OAuthProviderConfig>,
}

impl OAuthConfig {
    pub(super) fn from_env(bind_addr: SocketAddr) -> Result<Self, ConfigError> {
        let default_public = format!("http://localhost:{}", bind_addr.port());
        let public_api_url: String = optional("PUBLIC_API_URL", default_public)?;

        let google = match (
            non_empty("GOOGLE_CLIENT_ID"),
            non_empty("GOOGLE_CLIENT_SECRET"),
        ) {
            (Some(id), Some(secret)) => Some(OAuthProviderConfig::google(id, secret)),
            (None, None) => None,
            _ => return Err(both_or_neither("GOOGLE_CLIENT_ID")),
        };
        let github = match (
            non_empty("GITHUB_CLIENT_ID"),
            non_empty("GITHUB_CLIENT_SECRET"),
        ) {
            (Some(id), Some(secret)) => Some(OAuthProviderConfig::github(id, secret)),
            (None, None) => None,
            _ => return Err(both_or_neither("GITHUB_CLIENT_ID")),
        };

        Ok(Self {
            public_api_url: public_api_url.trim_end_matches('/').to_string(),
            google,
            github,
        })
    }
}

fn both_or_neither(name: &'static str) -> ConfigError {
    ConfigError::Invalid {
        name,
        reason: "the client id and the client secret must be set together".into(),
    }
}

/// WebAuthn relying party. Defaults are derived from `FRONTEND_URL`, because the browser (the
/// frontend origin) is what talks to the authenticator.
#[derive(Debug, Clone)]
pub struct WebauthnConfig {
    /// Registrable domain the passkeys are bound to, e.g. `localhost` or `example.com`.
    pub rp_id: String,
    /// Shown by the browser/OS when creating a passkey.
    pub rp_name: String,
    /// Exact frontend origin, e.g. `http://localhost:3001` (scheme + host + port).
    pub origin: String,
}

impl WebauthnConfig {
    pub(super) fn from_env(frontend_url: &str) -> Result<Self, ConfigError> {
        let parsed = url::Url::parse(frontend_url).map_err(|e| ConfigError::Invalid {
            name: "FRONTEND_URL",
            reason: e.to_string(),
        })?;
        let default_origin = parsed.origin().ascii_serialization();
        let default_rp_id = parsed.host_str().unwrap_or("localhost").to_string();

        Ok(Self {
            rp_id: optional("WEBAUTHN_RP_ID", default_rp_id)?,
            rp_name: optional("WEBAUTHN_RP_NAME", "api-starter-axum".to_string())?,
            origin: optional("WEBAUTHN_ORIGIN", default_origin)?,
        })
    }
}

/// `/metrics` is served on its own listener, outside the main app's router and middleware stack
/// (no CORS, no compression, not counted in `http_requests_total`).
#[derive(Clone)]
pub struct MetricsConfig {
    /// Defaults to loopback-only: safe by default, but a Dockerized Prometheus cannot reach it
    /// via `host.docker.internal` unless this is overridden to bind on `0.0.0.0`.
    pub bind_addr: SocketAddr,
    /// If set, `/metrics` requires `Authorization: Bearer <token>`.
    pub token: Option<String>,
}

impl fmt::Debug for MetricsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MetricsConfig")
            .field("bind_addr", &self.bind_addr)
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl MetricsConfig {
    pub(super) fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            bind_addr: optional("METRICS_BIND_ADDR", "127.0.0.1:9091".parse().unwrap())?,
            token: non_empty("METRICS_TOKEN"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct QrLoginConfig {
    /// How long a QR code stays valid (the client shows a fresh one afterwards).
    pub ttl_secs: i64,
    /// Polling is frequent by design, so it has its own, higher per-IP limit.
    pub poll_rate_limit_per_minute: u32,
}

impl QrLoginConfig {
    pub(super) fn from_env() -> Result<Self, ConfigError> {
        let ttl_secs: i64 = optional("QR_LOGIN_TTL_SECS", 120)?;
        if !(30..=900).contains(&ttl_secs) {
            return Err(ConfigError::Invalid {
                name: "QR_LOGIN_TTL_SECS",
                reason: "must be between 30 and 900".into(),
            });
        }
        Ok(Self {
            ttl_secs,
            poll_rate_limit_per_minute: optional("QR_POLL_RATE_LIMIT_PER_MINUTE", 120)?,
        })
    }
}
