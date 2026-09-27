//! Signing in with a passkey, usernameless (a two-step WebAuthn ceremony).

use super::PasskeysService;
use crate::modules::passkeys::{
    dto::{BeginResponse, FinishLoginDto},
    entity::ChallengeKind,
    error::PasskeyError,
};
use crate::{
    common::{
        error::{AppError, AppResult},
        security::jwt,
    },
    modules::auth::dto::TokenResponse,
};
use webauthn_rs::prelude::{DiscoverableAuthentication, DiscoverableKey, RequestChallengeResponse};

impl PasskeysService {
    /// Starts a usernameless sign-in: the browser lets the user pick one of their passkeys for
    /// this site, so nothing about any account is revealed to the caller.
    pub async fn begin_login(&self) -> AppResult<BeginResponse<RequestChallengeResponse>> {
        let (options, state) = self
            .webauthn
            .start_discoverable_authentication()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("webauthn login start: {e}")))?;
        let challenge_id = self
            .store_challenge(ChallengeKind::Authentication, None, &state)
            .await?;
        Ok(BeginResponse {
            challenge_id,
            options,
        })
    }

    pub async fn finish_login(&self, dto: FinishLoginDto) -> AppResult<TokenResponse> {
        let failed = || {
            metrics::counter!("passkey_ceremony_total", "kind" => "authentication", "outcome" => "failure")
                .increment(1);
            PasskeyError::AuthenticationFailed
        };

        let state: DiscoverableAuthentication = self
            .claim_challenge(&dto.challenge_id, ChallengeKind::Authentication, None)
            .await?
            .ok_or_else(failed)?;

        let (user_id, _) = self
            .webauthn
            .identify_discoverable_authentication(&dto.credential)
            .map_err(|_| failed())?;
        let user = self
            .users
            .find_by_id(user_id)
            .await?
            .filter(|u| u.is_active)
            .ok_or_else(failed)?;

        let mut passkeys = self.decoded(&self.repo.list_by_user(user.id).await?);
        let keys: Vec<DiscoverableKey> = passkeys
            .iter()
            .map(|(_, passkey)| DiscoverableKey::from(passkey))
            .collect();
        let result = self
            .webauthn
            .finish_discoverable_authentication(&dto.credential, state, &keys)
            .map_err(|error| {
                tracing::warn!(%error, "passkey sign-in rejected");
                failed()
            })?;

        // Keep the signature counter current: a regressing counter signals a cloned authenticator.
        for (row, passkey) in &mut passkeys {
            if passkey.update_credential(&result).is_some() {
                let json = serde_json::to_string(passkey).map_err(anyhow::Error::from)?;
                self.repo.update_after_use(row.id, &json).await?;
            }
        }

        if self.require_verified_email && !user.is_email_verified() {
            return Err(PasskeyError::EmailNotVerified.into());
        }
        let access_token = jwt::issue(
            user.id,
            user.role,
            user.token_version,
            &self.jwt.secret,
            self.jwt.ttl_secs,
        )?;
        metrics::counter!("passkey_ceremony_total", "kind" => "authentication", "outcome" => "success")
            .increment(1);
        Ok(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in: self.jwt.ttl_secs,
        })
    }
}
