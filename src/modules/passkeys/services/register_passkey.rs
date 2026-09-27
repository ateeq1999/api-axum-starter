//! Registering a passkey on a signed-in account (a two-step WebAuthn ceremony).

use super::{MAX_PASSKEYS_PER_USER, PasskeysService};
use crate::common::{
    error::{AppError, AppResult},
    security::SessionUser,
};
use crate::modules::passkeys::{
    dto::{BeginResponse, FinishRegistrationDto, PasskeyResponse},
    entity::ChallengeKind,
    error::PasskeyError,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use webauthn_rs::prelude::{CreationChallengeResponse, PasskeyRegistration};

impl PasskeysService {
    pub async fn begin_registration(
        &self,
        actor: &SessionUser,
    ) -> AppResult<BeginResponse<CreationChallengeResponse>> {
        let user = self.active_user(actor.0.id).await?;
        let existing = self.decoded(&self.repo.list_by_user(user.id).await?);
        if existing.len() >= MAX_PASSKEYS_PER_USER {
            return Err(PasskeyError::LimitReached(MAX_PASSKEYS_PER_USER).into());
        }

        // Ask the authenticator not to create a second passkey for a device that already has one.
        let exclude = existing
            .iter()
            .map(|(_, passkey)| passkey.cred_id().clone())
            .collect();
        let display_name = user
            .display_name
            .clone()
            .unwrap_or_else(|| user.email.clone());
        let (options, state) = self
            .webauthn
            .start_passkey_registration(user.id, &user.email, &display_name, Some(exclude))
            .map_err(|e| AppError::Internal(anyhow::anyhow!("webauthn registration start: {e}")))?;

        let challenge_id = self
            .store_challenge(ChallengeKind::Registration, Some(user.id), &state)
            .await?;
        Ok(BeginResponse {
            challenge_id,
            options,
        })
    }

    pub async fn finish_registration(
        &self,
        actor: &SessionUser,
        dto: FinishRegistrationDto,
    ) -> AppResult<PasskeyResponse> {
        let state: PasskeyRegistration = self
            .claim_challenge(
                &dto.challenge_id,
                ChallengeKind::Registration,
                Some(actor.0.id),
            )
            .await?
            .ok_or(PasskeyError::RegistrationFailed)?;

        let passkey = self
            .webauthn
            .finish_passkey_registration(&dto.credential, &state)
            .map_err(|error| {
                tracing::warn!(%error, "passkey registration rejected");
                PasskeyError::RegistrationFailed
            })?;

        let name = dto.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
        let stored = self
            .repo
            .insert(
                actor.0.id,
                name.unwrap_or("Passkey"),
                &URL_SAFE_NO_PAD.encode(passkey.cred_id()),
                &serde_json::to_string(&passkey).map_err(anyhow::Error::from)?,
            )
            .await?
            .ok_or(PasskeyError::AlreadyRegistered)?;
        metrics::counter!("passkey_ceremony_total", "kind" => "registration", "outcome" => "success")
            .increment(1);
        Ok(stored.into())
    }
}
