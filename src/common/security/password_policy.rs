//! Password strength scoring (`zxcvbn`) and optional breach checking (Have I Been Pwned's
//! k-anonymity API), independent of the plain length check already done at the DTO layer.

use sha1::{Digest, Sha1};
use zxcvbn::{Score, zxcvbn};

/// Below this, a password is rejected outright as too easily guessable. `Score::Two` means
/// crackable in 10^8 guesses or fewer (see `zxcvbn::Score`'s own doc comments) — strict enough to
/// reject dictionary-word-plus-digits passwords like "password123", lenient enough not to reject
/// a reasonably long, non-trivial passphrase.
const MINIMUM_STRENGTH_SCORE: Score = Score::Two;

/// Checks password strength, given whatever about the account is already known (email, display
/// name, ...) so a password built from them is penalized rather than judged in isolation — e.g.
/// "alice2024" for alice@example.com scores far lower than it would without that context.
/// Returns a human-readable rejection reason on failure.
pub fn reject_if_too_weak(password: &str, known_inputs: &[&str]) -> Result<(), String> {
    let strength_estimate = zxcvbn(password, known_inputs);
    if strength_estimate.score() >= MINIMUM_STRENGTH_SCORE {
        return Ok(());
    }
    let rejection_reason = strength_estimate
        .feedback()
        .and_then(|feedback| feedback.warning())
        .map(|warning| warning.to_string())
        .unwrap_or_else(|| "This password is too easy to guess.".to_string());
    Err(rejection_reason)
}

/// Checks a password against the Have I Been Pwned breached-password database using the
/// k-anonymity API: only the first 5 hex characters of the password's SHA-1 hash are ever sent,
/// so the real password (and its full hash) never leaves this process. Opt-in via
/// `CHECK_PASSWORD_BREACHES` — an external service outage must never block registration or
/// login, so any network/parse failure here is treated as "not found", not as a hard error.
pub async fn has_appeared_in_a_known_breach(http_client: &reqwest::Client, password: &str) -> bool {
    let full_hash = hex_upper(&Sha1::digest(password.as_bytes()));
    let (hash_prefix, hash_suffix) = full_hash.split_at(5);

    let request = http_client
        .get(format!(
            "https://api.pwnedpasswords.com/range/{hash_prefix}"
        ))
        .header("Add-Padding", "true")
        .send();
    let response = match tokio::time::timeout(std::time::Duration::from_secs(3), request).await {
        Ok(Ok(response)) => response,
        _ => {
            tracing::warn!("could not reach the password-breach check service, skipping it");
            return false;
        }
    };
    let Ok(body) = response.text().await else {
        return false;
    };

    body.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(suffix, _count)| suffix.eq_ignore_ascii_case(hash_suffix))
    })
}

fn hex_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_common_dictionary_based_passwords() {
        assert!(reject_if_too_weak("password123", &[]).is_err());
        assert!(reject_if_too_weak("qwertyuiop", &[]).is_err());
    }

    #[test]
    fn accepts_a_reasonably_strong_passphrase() {
        assert!(reject_if_too_weak("correct-horse-battery", &[]).is_ok());
    }

    #[test]
    fn penalizes_passwords_built_from_known_account_details() {
        let email = "alice@example.com";
        // Weak specifically because it is built from the account's own email.
        assert!(reject_if_too_weak("alice2024", &[email]).is_err());
    }
}
