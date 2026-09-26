//! Configuration of the optional feature modules: uploads, OAuth, WebAuthn, QR login.

use std::{fmt, net::SocketAddr, path::PathBuf};

use super::{ConfigError, non_empty, optional};

#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Where profile photos are stored on local disk (created at startup). Ignored when
    /// `s3` is set.
    pub upload_dir: PathBuf,
    /// When set, uploads go to this S3 bucket instead of local disk.
    pub s3: Option<S3StorageConfig>,
}

/// S3 credentials, region and (for emulators) endpoint come from the standard AWS
/// environment variables (`AWS_ACCESS_KEY_ID`, `AWS_REGION`, `AWS_ENDPOINT_URL`, ...), or an
/// instance role on EC2, which the AWS SDK resolves itself.
#[derive(Debug, Clone)]
pub struct S3StorageConfig {
    pub bucket: String,
    /// `endpoint/bucket/key` addressing instead of `bucket.endpoint/key`. Local S3 emulators
    /// (floci, LocalStack, MinIO) need it, so it defaults to on whenever `AWS_ENDPOINT_URL`
    /// points somewhere other than real AWS.
    pub force_path_style: bool,
    /// `AWS_ENDPOINT_URL` when set and non-blank (a local emulator). Passed to the SDK explicitly
    /// so a blank value in `.env` means "real AWS" instead of an invalid endpoint.
    pub endpoint_url: Option<String>,
}

impl StorageConfig {
    pub(super) fn from_env() -> Result<Self, ConfigError> {
        let s3 = match non_empty("S3_BUCKET") {
            Some(bucket) => Some(S3StorageConfig {
                bucket,
                force_path_style: optional(
                    "S3_FORCE_PATH_STYLE",
                    non_empty("AWS_ENDPOINT_URL").is_some(),
                )?,
                endpoint_url: non_empty("AWS_ENDPOINT_URL"),
            }),
            None => None,
        };
        Ok(Self {
            upload_dir: optional("UPLOAD_DIR", PathBuf::from("./uploads"))?,
            s3,
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

/// Limits for user-uploaded media (`/media`).
#[derive(Debug, Clone)]
pub struct MediaConfig {
    /// Largest single upload accepted. Must not exceed `MAX_REQUEST_BODY_BYTES`.
    pub max_upload_bytes: usize,
    /// Content types a user may upload. Checked against the file's actual bytes (magic
    /// numbers), never against the client-declared `Content-Type`. Types that browsers execute
    /// or render actively (HTML, SVG, scripts) are deliberately absent from the default list.
    pub allowed_content_types: Vec<String>,
}

const DEFAULT_ALLOWED_MEDIA_TYPES: &str = "image/jpeg,image/png,image/gif,image/webp,\
    application/pdf,video/mp4,video/webm,audio/mpeg,audio/ogg";

impl MediaConfig {
    pub(super) fn from_env(max_request_body_bytes: usize) -> Result<Self, ConfigError> {
        let max_upload_bytes: usize = optional("MEDIA_MAX_UPLOAD_BYTES", 8 * 1024 * 1024)?;
        if max_upload_bytes > max_request_body_bytes {
            return Err(ConfigError::Invalid {
                name: "MEDIA_MAX_UPLOAD_BYTES",
                reason: format!(
                    "{max_upload_bytes} exceeds MAX_REQUEST_BODY_BYTES ({max_request_body_bytes}), \
                     so uploads that large would be rejected before reaching the media handler"
                ),
            });
        }
        let raw_types = non_empty("MEDIA_ALLOWED_CONTENT_TYPES")
            .unwrap_or_else(|| DEFAULT_ALLOWED_MEDIA_TYPES.to_string());
        let allowed_content_types = raw_types
            .split(',')
            .map(|content_type| content_type.trim().to_ascii_lowercase())
            .filter(|content_type| !content_type.is_empty())
            .collect();
        Ok(Self {
            max_upload_bytes,
            allowed_content_types,
        })
    }
}
