use std::{env, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub jwt_secret: String,
    pub jwt_ttl_secs: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable `{0}`")]
    Missing(&'static str),
    #[error("invalid value for `{name}`: {reason}")]
    Invalid { name: &'static str, reason: String },
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError::Missing(name))
}

fn optional<T>(name: &'static str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
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

        Ok(Self {
            database_url: required("DATABASE_URL")?,
            bind_addr: optional("BIND_ADDR", "127.0.0.1:3000".parse().unwrap())?,
            jwt_secret,
            jwt_ttl_secs: optional("JWT_TTL_SECS", 3600)?,
        })
    }
}
