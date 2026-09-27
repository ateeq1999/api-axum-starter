//! The provider's redirect back: sign in, sign up, or link, by what the state says.

use super::{CallbackOutcome, GRANT_TTL, OAuthService, provider_error, server_error};
use crate::common::security::secret_token;
use crate::modules::oauth::{dto::CallbackQuery, entity::ProviderProfile, provider::Provider};
use chrono::Utc;
use uuid::Uuid;

impl OAuthService {
    /// Handles the provider's redirect. Never returns an error: every failure becomes a
    /// [`CallbackOutcome::Failed`] so the browser lands on a page instead of a JSON error.
    pub async fn handle_callback(
        &self,
        provider: Provider,
        query: CallbackQuery,
    ) -> CallbackOutcome {
        let outcome = match self.callback(provider, query).await {
            Ok(outcome) => outcome,
            Err(reason) => CallbackOutcome::Failed(reason),
        };
        let label = match &outcome {
            CallbackOutcome::LoggedIn { .. } => "logged_in",
            CallbackOutcome::Linked { .. } => "linked",
            CallbackOutcome::Failed(reason) => reason,
        };
        metrics::counter!("oauth_login_total", "provider" => provider.as_str(), "outcome" => label)
            .increment(1);
        outcome
    }

    async fn callback(
        &self,
        provider: Provider,
        query: CallbackQuery,
    ) -> Result<CallbackOutcome, &'static str> {
        let state_raw = query.state.as_deref().ok_or("invalid_state")?;
        let state = self
            .repo
            .claim_state(&secret_token::hash(state_raw))
            .await
            .map_err(server_error)?
            .ok_or("invalid_state")?;
        if state.provider != provider {
            return Err("invalid_state");
        }
        if query.error.is_some() {
            return Err("access_denied");
        }
        let code = query.code.as_deref().ok_or("access_denied")?;

        let access_token = self
            .client
            .exchange_code(provider, code, &state.pkce_verifier)
            .await
            .map_err(provider_error)?;
        let profile = self
            .client
            .fetch_profile(provider, &access_token)
            .await
            .map_err(provider_error)?;

        self.resolve(provider, profile, state.user_id, state.redirect_path)
            .await
    }

    /// Decides which account this provider identity belongs to.
    async fn resolve(
        &self,
        provider: Provider,
        profile: ProviderProfile,
        linking_user: Option<Uuid>,
        redirect: String,
    ) -> Result<CallbackOutcome, &'static str> {
        // 1. Already linked: sign that account in (or confirm the link).
        if let Some(identity) = self
            .repo
            .find_identity(provider, &profile.provider_user_id)
            .await
            .map_err(server_error)?
        {
            return match linking_user {
                Some(user_id) if user_id == identity.user_id => {
                    Ok(CallbackOutcome::Linked { provider, redirect })
                }
                Some(_) => Err("already_linked"),
                None => self.grant_for(identity.user_id, redirect).await,
            };
        }

        // 2. A signed-in user is adding this provider to their account.
        if let Some(user_id) = linking_user {
            self.users
                .find_by_id(user_id)
                .await
                .map_err(server_error)?
                .filter(|u| u.is_active)
                .ok_or("invalid_state")?;
            self.link(user_id, provider, &profile).await?;
            return Ok(CallbackOutcome::Linked { provider, redirect });
        }

        // 3. Sign in or sign up by verified email. Never trust an unverified provider email.
        let email = profile
            .email
            .as_deref()
            .filter(|_| profile.email_verified)
            .ok_or("email_not_verified")?;

        let user = match self
            .users
            .find_by_email(email)
            .await
            .map_err(server_error)?
        {
            Some(existing) => {
                if !existing.is_active {
                    return Err("account_disabled");
                }
                // Linking to an account whose email was never verified would let whoever
                // pre-registered that address (without owning it) share the account.
                if !existing.is_email_verified() {
                    return Err("account_exists_unverified");
                }
                existing
            }
            None => {
                let display_name = profile
                    .name
                    .as_deref()
                    .map(|n| n.trim().chars().take(100).collect::<String>())
                    .filter(|n| !n.is_empty());
                self.users
                    .create_passwordless(email, display_name.as_deref(), true)
                    .await
                    .map_err(server_error)?
            }
        };
        self.link(user.id, provider, &profile).await?;
        self.grant_for(user.id, redirect).await
    }

    async fn link(
        &self,
        user_id: Uuid,
        provider: Provider,
        profile: &ProviderProfile,
    ) -> Result<(), &'static str> {
        let linked = self
            .repo
            .insert_identity(
                user_id,
                provider,
                &profile.provider_user_id,
                profile.email.as_deref(),
            )
            .await
            .map_err(server_error)?;
        if linked {
            Ok(())
        } else {
            Err("already_linked")
        }
    }

    /// Issues the single-use code the SPA trades for an access token.
    async fn grant_for(
        &self,
        user_id: Uuid,
        redirect: String,
    ) -> Result<CallbackOutcome, &'static str> {
        let user = self
            .users
            .find_by_id(user_id)
            .await
            .map_err(server_error)?
            .ok_or("invalid_state")?;
        if !user.is_active {
            return Err("account_disabled");
        }
        if self.require_verified_email && !user.is_email_verified() {
            return Err("email_not_verified");
        }

        let grant = secret_token::generate();
        self.repo
            .insert_grant(&grant.hash, user_id, Utc::now() + GRANT_TTL)
            .await
            .map_err(server_error)?;
        Ok(CallbackOutcome::LoggedIn {
            code: grant.raw,
            redirect,
        })
    }
}
