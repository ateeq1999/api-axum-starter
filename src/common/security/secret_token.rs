use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

pub struct GeneratedToken {
    /// Sent to the user (inside a link). Never stored.
    pub raw: String,
    /// What the database keeps: SHA-256 of `raw`, hex encoded.
    pub hash: String,
}

/// 32 random bytes, base64url encoded (43 chars).
pub fn generate() -> GeneratedToken {
    let bytes: [u8; 32] = rand::random();
    let raw = URL_SAFE_NO_PAD.encode(bytes);
    let hash = hash(&raw);
    GeneratedToken { raw, hash }
}

/// The raw token is high-entropy random data, so a fast hash is enough (unlike passwords).
pub fn hash(raw: &str) -> String {
    Sha256::digest(raw.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// PKCE `S256` code challenge for a verifier: base64url(SHA-256(verifier)) (RFC 7636).
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_hash_matches() {
        let a = generate();
        let b = generate();
        assert_ne!(a.raw, b.raw);
        assert_eq!(a.raw.len(), 43);
        assert_eq!(a.hash, hash(&a.raw));
        assert_eq!(a.hash.len(), 64);
        assert_ne!(a.hash, a.raw);
    }

    #[test]
    fn pkce_challenge_matches_the_rfc_example() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(
            hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
