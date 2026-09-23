//! Thin, descriptively-named wrapper around `totp-rs` (RFC 6238 TOTP), so callers never touch the
//! underlying `TOTP`/`Secret` types directly.

use totp_rs::{Algorithm, Secret, TOTP};

const CODE_DIGITS: usize = 6;
/// Accepts the previous and next 30-second window too, to tolerate clock drift between the
/// server and the user's phone.
const ALLOWED_CLOCK_SKEW_STEPS: u8 = 1;
const TIME_STEP_SECONDS: u64 = 30;

/// A freshly generated secret, ready to be shown to the user (as a QR code and as text) and
/// persisted (`base32_secret`, e.g. in `users.totp_secret`) before it is confirmed.
pub struct GeneratedAuthenticatorSecret {
    /// Store this. Base32-encoded, so it is also typeable by hand if the QR code cannot be
    /// scanned.
    pub base32_secret: String,
    /// `data:image/png;base64,...`, ready to use directly as an `<img src>`.
    pub qr_code_data_uri: String,
    /// The same information as the QR code, as a URI, for authenticator apps that accept manual
    /// entry via a link instead of scanning.
    pub provisioning_uri: String,
}

fn build_authenticator(
    base32_secret: &str,
    issuer: &str,
    account_email: &str,
) -> anyhow::Result<TOTP> {
    let secret_bytes = Secret::Encoded(base32_secret.to_string())
        .to_bytes()
        .map_err(|e| anyhow::anyhow!("invalid TOTP secret encoding: {e}"))?;
    TOTP::new(
        Algorithm::SHA1, // the only algorithm every mainstream authenticator app supports
        CODE_DIGITS,
        ALLOWED_CLOCK_SKEW_STEPS,
        TIME_STEP_SECONDS,
        secret_bytes,
        Some(issuer.to_string()),
        account_email.to_string(),
    )
    .map_err(|e| anyhow::anyhow!("could not build TOTP authenticator: {e}"))
}

/// Generates a new secret and everything needed to enroll an authenticator app in it. The
/// account is not protected by it yet — the caller must still verify a code from it (see
/// [`verify_code`]) before treating two-factor authentication as enabled.
pub fn generate_authenticator_secret(
    issuer: &str,
    account_email: &str,
) -> anyhow::Result<GeneratedAuthenticatorSecret> {
    let base32_secret = Secret::generate_secret().to_encoded().to_string();
    let authenticator = build_authenticator(&base32_secret, issuer, account_email)?;
    Ok(GeneratedAuthenticatorSecret {
        qr_code_data_uri: format!(
            "data:image/png;base64,{}",
            authenticator
                .get_qr_base64()
                .map_err(|e| anyhow::anyhow!("could not render TOTP QR code: {e}"))?
        ),
        provisioning_uri: authenticator.get_url(),
        base32_secret,
    })
}

/// Checks a 6-digit code from the authenticator app against the stored secret.
pub fn verify_code(base32_secret: &str, issuer: &str, account_email: &str, code: &str) -> bool {
    let Ok(authenticator) = build_authenticator(base32_secret, issuer, account_email) else {
        return false;
    };
    authenticator.check_current(code).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_freshly_generated_secret_verifies_its_own_current_code() {
        let generated =
            generate_authenticator_secret("api-starter-axum", "user@example.com").unwrap();
        assert!(
            generated
                .qr_code_data_uri
                .starts_with("data:image/png;base64,")
        );
        assert!(generated.provisioning_uri.starts_with("otpauth://totp/"));

        let authenticator = build_authenticator(
            &generated.base32_secret,
            "api-starter-axum",
            "user@example.com",
        )
        .unwrap();
        let current_code = authenticator.generate_current().unwrap();
        assert!(verify_code(
            &generated.base32_secret,
            "api-starter-axum",
            "user@example.com",
            &current_code,
        ));
    }

    #[test]
    fn rejects_a_wrong_code() {
        let generated =
            generate_authenticator_secret("api-starter-axum", "user@example.com").unwrap();
        assert!(!verify_code(
            &generated.base32_secret,
            "api-starter-axum",
            "user@example.com",
            "000000",
        ));
    }
}
