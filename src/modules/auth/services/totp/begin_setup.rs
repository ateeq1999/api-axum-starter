//! Starting two-factor enrollment: a new secret and its QR code.

use super::TotpService;
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, totp},
    },
    modules::auth::dto::TotpSetupResponse,
};

impl TotpService {
    /// Starts (or restarts) enrollment: generates a new secret and stores it, but two-factor is
    /// not enforced at login until [`Self::confirm_setup`] proves the app has it too.
    pub async fn begin_setup(&self, actor: &AuthUser) -> AppResult<TotpSetupResponse> {
        let user = self.require_active_user(actor.id).await?;
        let generated = totp::generate_authenticator_secret(&self.issuer, &user.email)?;
        self.users
            .set_pending_totp_secret(user.id, &generated.base32_secret)
            .await?;
        Ok(TotpSetupResponse {
            secret: generated.base32_secret,
            qr_code_data_uri: generated.qr_code_data_uri,
            provisioning_uri: generated.provisioning_uri,
        })
    }
}
