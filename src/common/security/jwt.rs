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
    pub iat: i64,
    pub exp: i64,
}

pub fn issue(user_id: Uuid, role: Role, secret: &str, ttl_secs: i64) -> AppResult<String> {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user_id,
        role,
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
    fn roundtrip_keeps_subject_and_role() {
        let id = Uuid::new_v4();
        let token = issue(id, Role::Admin, SECRET, 60).unwrap();
        let claims = verify(&token, SECRET).unwrap();
        assert_eq!(claims.sub, id);
        assert_eq!(claims.role, Role::Admin);
    }

    #[test]
    fn rejects_wrong_secret_and_expired() {
        let id = Uuid::new_v4();
        let token = issue(id, Role::User, SECRET, 60).unwrap();
        assert!(verify(&token, "another-secret-another-secret-123").is_err());
        let expired = issue(id, Role::User, SECRET, -120).unwrap();
        assert!(verify(&expired, SECRET).is_err());
    }
}
