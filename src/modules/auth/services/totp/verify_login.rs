//! The second sign-in step: an app code or a recovery code.

use super::TotpService;
use crate::{
    common::{
        error::AppResult,
        security::{jwt, secret_token, totp},
    },
    modules::auth::{dto::TokenResponse, entity::TokenPurpose, error::AuthError},
};

impl TotpService {
    /// Completes a login that a correct password alone was not enough for. `code` may be a live
    /// authenticator code or an unused recovery code.
    pub async fn verify_login(&self, pending_token: &str, code: &str) -> AppResult<TokenResponse> {
        let failed = || AuthError::InvalidTotpCode;

        // Peeked, not consumed yet: a mistyped code must not burn the whole pending login, so the
        // same pending_token stays usable for another attempt within its TTL. Only a successful
        // code (below) actually spends it.
        let claim = self
            .tokens
            .peek(TokenPurpose::TwoFactorPending, pending_token)
            .await?;
        let user = self
            .users
            .find_by_id(claim.user_id)
            .await?
            .filter(|u| u.is_active && u.totp_enabled)
            .ok_or_else(failed)?;
        let secret = user.totp_secret.as_deref().ok_or_else(failed)?;

        let code_is_valid = totp::verify_code(secret, &self.issuer, &user.email, code)
            || self
                .recovery_codes
                .claim(user.id, &secret_token::hash(code))
                .await?;
        if !code_is_valid {
            metrics::counter!("auth_totp_verify_total", "outcome" => "failure").increment(1);
            return Err(failed().into());
        }
        self.tokens
            .redeem(TokenPurpose::TwoFactorPending, pending_token)
            .await?;

        let access_token = jwt::issue(
            user.id,
            user.role,
            user.token_version,
            &self.jwt.secret,
            self.jwt.ttl_secs,
        )?;
        metrics::counter!("auth_totp_verify_total", "outcome" => "success").increment(1);
        Ok(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in: self.jwt.ttl_secs,
        })
    }
}
