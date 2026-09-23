use std::sync::Arc;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use uuid::Uuid;
use webauthn_rs::prelude::{
    CreationChallengeResponse, DiscoverableAuthentication, DiscoverableKey, Passkey,
    PasskeyRegistration, RequestChallengeResponse, Url, Webauthn, WebauthnBuilder,
};

use super::{
    dto::{BeginResponse, FinishLoginDto, FinishRegistrationDto, PasskeyResponse},
    entity::{ChallengeKind, PasskeyRow},
    error::PasskeyError,
    repository::PasskeysRepository,
};
use crate::{
    common::{
        error::{AppError, AppResult},
        security::{JwtSettings, SessionUser, jwt},
    },
    config::WebauthnConfig,
    modules::{auth::dto::TokenResponse, users::UsersService},
};

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

    // ---- registering a passkey (signed in) ---------------------------------------------

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

    // ---- signing in with a passkey (public) --------------------------------------------

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

    // ---- managing passkeys (signed in) -------------------------------------------------

    pub async fn list(&self, actor: &SessionUser) -> AppResult<Vec<PasskeyResponse>> {
        Ok(self
            .repo
            .list_by_user(actor.0.id)
            .await?
            .into_iter()
            .map(PasskeyResponse::from)
            .collect())
    }

    /// Removes a passkey, unless it is the account's only way to sign in.
    pub async fn delete(&self, actor: &SessionUser, id: Uuid) -> AppResult<()> {
        let owns_it = self
            .repo
            .list_by_user(actor.0.id)
            .await?
            .iter()
            .any(|p| p.id == id);
        if !owns_it {
            return Err(PasskeyError::NotFound.into());
        }
        if self.users.sign_in_method_count(actor.0.id).await? < 2 {
            return Err(PasskeyError::LastSignInMethod.into());
        }
        self.repo.delete(actor.0.id, id).await?;
        Ok(())
    }

    // ---- helpers -----------------------------------------------------------------------

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
