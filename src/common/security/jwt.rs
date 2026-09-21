use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub iat: i64,
    pub exp: i64,
}

pub fn issue(user_id: Uuid, secret: &str, ttl_secs: i64) -> AppResult<String> {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user_id,
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

pub fn verify(token: &str, secret: &str) -> AppResult<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized("invalid or expired token"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn roundtrip() {
        let id = Uuid::new_v4();
        let token = issue(id, SECRET, 60).unwrap();
        assert_eq!(verify(&token, SECRET).unwrap().sub, id);
    }

    #[test]
    fn rejects_wrong_secret_and_expired() {
        let id = Uuid::new_v4();
        let token = issue(id, SECRET, 60).unwrap();
        assert!(verify(&token, "another-secret-another-secret-123").is_err());
        let expired = issue(id, SECRET, -120).unwrap();
        assert!(verify(&expired, SECRET).is_err());
    }
}
