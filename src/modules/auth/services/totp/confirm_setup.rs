//! Turning two-factor on with a code from the app, and issuing recovery codes.

use super::TotpService;
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, totp},
    },
    modules::auth::{dto::TotpEnabledResponse, error::AuthError, helpers::recovery_code},
};

impl TotpService {
    /// Confirms enrollment: the code must come from the secret [`Self::begin_setup`] just issued.
    /// Turns two-factor on and returns a fresh batch of recovery codes, shown to the caller
    /// exactly once.
    pub async fn confirm_setup(
        &self,
        actor: &AuthUser,
        code: &str,
    ) -> AppResult<TotpEnabledResponse> {
        let user = self.require_active_user(actor.id).await?;
        if user.totp_enabled {
            return Err(AuthError::TotpAlreadyEnabled.into());
        }
        let pending_secret = user
            .totp_secret
            .as_deref()
            .ok_or(AuthError::NoTotpSetupInProgress)?;
        if !totp::verify_code(pending_secret, &self.issuer, &user.email, code) {
            return Err(AuthError::InvalidTotpCode.into());
        }

        self.users.enable_totp(user.id).await?;
        let batch = recovery_code::generate_batch();
        let hashes: Vec<String> = batch.iter().map(|c| c.hash.clone()).collect();
        self.recovery_codes.replace_all(user.id, &hashes).await?;

        Ok(TotpEnabledResponse {
            recovery_codes: batch.into_iter().map(|c| c.raw).collect(),
        })
    }
}
