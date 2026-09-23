use std::fmt;

use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{error::SecurityError, role::Role};
use crate::common::error::{AppError, AppResult};

#[derive(Clone)]
pub struct JwtSettings {
    pub secret: String,
    pub ttl_secs: i64,
}

impl fmt::Debug for JwtSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JwtSettings")
            .field("secret", &"<redacted>")
            .field("ttl_secs", &self.ttl_secs)
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    #[serde(default)]
    pub role: Role,
    /// The user's `token_version` at issue time. Checked live against the database on every
    /// request (see `SessionVerifier`), so a password change invalidates tokens issued before it
    /// immediately, instead of leaving them valid until they expire.
    #[serde(default = "default_token_version")]
    pub tv: i32,
    pub iat: i64,
    pub exp: i64,
}

fn default_token_version() -> i32 {
    1
}

pub fn issue(
    user_id: Uuid,
    role: Role,
    token_version: i32,
    secret: &str,
    ttl_secs: i64,
) -> AppResult<String> {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user_id,
        role,
        tv: token_version,
        iat: now,
        exp: now + ttl_secs,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("jwt encode failed: {e}")))
}

pub fn verify(token: &str, secret: &str) -> Result<Claims, SecurityError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| SecurityError::InvalidToken)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn roundtrip_keeps_subject_role_and_token_version() {
        let id = Uuid::new_v4();
        let token = issue(id, Role::Admin, 3, SECRET, 60).unwrap();
        let claims = verify(&token, SECRET).unwrap();
        assert_eq!(claims.sub, id);
        assert_eq!(claims.role, Role::Admin);
        assert_eq!(claims.tv, 3);
    }

    #[test]
    fn rejects_wrong_secret_and_expired() {
        let id = Uuid::new_v4();
        let token = issue(id, Role::User, 1, SECRET, 60).unwrap();
        assert!(verify(&token, "another-secret-another-secret-123").is_err());
        let expired = issue(id, Role::User, 1, SECRET, -120).unwrap();
        assert!(verify(&expired, SECRET).is_err());
    }
}
