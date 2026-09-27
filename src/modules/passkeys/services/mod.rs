use std::sync::Arc;

use chrono::{Duration, Utc};
use uuid::Uuid;
use webauthn_rs::prelude::{Passkey, Url, Webauthn, WebauthnBuilder};

use super::{
    entity::{ChallengeKind, PasskeyRow},
    repositories::PasskeysRepository,
};
use crate::{
    common::{error::AppResult, security::JwtSettings},
    config::WebauthnConfig,
    modules::users::UsersService,
};

mod delete_passkey;
mod list_passkeys;
mod register_passkey;
mod sign_in_with_passkey;

const CHALLENGE_TTL: Duration = Duration::minutes(5);
const MAX_PASSKEYS_PER_USER: usize = 10;

#[derive(Clone)]
pub struct PasskeysService {
    webauthn: Arc<Webauthn>,
    repo: PasskeysRepository,
    users: UsersService,
    jwt: JwtSettings,
    require_verified_email: bool,
}

impl PasskeysService {
    /// Fails at startup if the relying party is misconfigured (e.g. `WEBAUTHN_RP_ID` is not a
    /// domain suffix of `WEBAUTHN_ORIGIN`'s host).
    pub fn new(
        config: &WebauthnConfig,
        repo: PasskeysRepository,
        users: UsersService,
        jwt: JwtSettings,
        require_verified_email: bool,
    ) -> anyhow::Result<Self> {
        let origin = Url::parse(&config.origin).map_err(|e| {
            anyhow::anyhow!("WEBAUTHN_ORIGIN `{}` is not a URL: {e}", config.origin)
        })?;
        let webauthn = WebauthnBuilder::new(&config.rp_id, &origin)
            .map_err(|e| {
                anyhow::anyhow!(
                    "invalid WebAuthn config (rp id `{}`, origin `{origin}`): {e}",
                    config.rp_id
                )
            })?
            .rp_name(&config.rp_name)
            .build()
            .map_err(|e| anyhow::anyhow!("could not build WebAuthn: {e}"))?;
        Ok(Self {
            webauthn: Arc::new(webauthn),
            repo,
            users,
            jwt,
            require_verified_email,
        })
    }

    async fn active_user(&self, id: Uuid) -> AppResult<crate::modules::users::User> {
        Ok(self
            .users
            .find_by_id(id)
            .await?
            .filter(|u| u.is_active)
            .ok_or(crate::modules::users::UsersError::NotFound)?)
    }

    async fn store_challenge<T: serde::Serialize>(
        &self,
        kind: ChallengeKind,
        user_id: Option<Uuid>,
        state: &T,
    ) -> AppResult<String> {
        let id = Uuid::new_v4().simple().to_string();
        self.repo
            .insert_challenge(
                &id,
                kind,
                user_id,
                &serde_json::to_string(state).map_err(anyhow::Error::from)?,
                Utc::now() + CHALLENGE_TTL,
            )
            .await?;
        Ok(id)
    }

    async fn claim_challenge<T: serde::de::DeserializeOwned>(
        &self,
        id: &str,
        kind: ChallengeKind,
        user_id: Option<Uuid>,
    ) -> AppResult<Option<T>> {
        let Some(json) = self.repo.claim_challenge(id, kind, user_id).await? else {
            return Ok(None);
        };
        Ok(Some(
            serde_json::from_str(&json).map_err(anyhow::Error::from)?,
        ))
    }

    /// Stored rows with their decoded credential. Rows that no longer decode are skipped
    /// (and logged) rather than breaking sign-in for the user's other passkeys.
    fn decoded(&self, rows: &[PasskeyRow]) -> Vec<(PasskeyRow, Passkey)> {
        rows.iter()
            .filter_map(|row| match serde_json::from_str::<Passkey>(&row.credential_json) {
                Ok(passkey) => Some((row.clone(), passkey)),
                Err(error) => {
                    tracing::error!(%error, passkey_id = %row.id, "stored passkey is unreadable");
                    None
                }
            })
            .collect()
    }
}
