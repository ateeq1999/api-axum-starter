use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};

use crate::error::{AppError, AppResult};

pub fn hash(password: &str) -> AppResult<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("password hashing failed: {e}")))
}

pub fn verify(password: &str, stored_hash: &str) -> bool {
    PasswordHash::new(stored_hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify() {
        let h = hash("correct horse").unwrap();
        assert!(verify("correct horse", &h));
        assert!(!verify("wrong", &h));
    }
}
