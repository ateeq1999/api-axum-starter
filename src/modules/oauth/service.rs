use chrono::{Duration, Utc};
use uuid::Uuid;

use super::{
    client::ProviderClient,
    dto::{AuthorizeUrlResponse, CallbackQuery, IdentityResponse, ProviderInfo},
    entity::ProviderProfile,
    error::OAuthError,
    provider::Provider,
    repository::OAuthRepository,
};
use crate::{
    common::{
        error::{AppError, AppResult},
        security::{JwtSettings, SessionUser, jwt, secret_token},
    },
    modules::{auth::dto::TokenResponse, users::UsersService},
};

const STATE_TTL: Duration = Duration::minutes(10);
/// The redirect-to-token hand-over is immediate, so the code only needs to live briefly.
const GRANT_TTL: Duration = Duration::seconds(60);

/// What the browser should be told after the provider redirects back.
#[derive(Debug, PartialEq, Eq)]
pub enum CallbackOutcome {
    /// Signed in: the SPA exchanges `code` for an access token.
    LoggedIn { code: String, redirect: String },
    /// An existing account was linked to the provider.
    Linked {
        provider: Provider,
        redirect: String,
    },
    /// Something went wrong; `reason` is a stable machine-readable code for the UI.
    Failed(&'static str),
}

#[derive(Clone)]
pub struct OAuthService {
    repo: OAuthRepository,
    users: UsersService,
    client: ProviderClient,
    jwt: JwtSettings,
    require_verified_email: bool,
}

impl OAuthService {
    pub fn new(
        repo: OAuthRepository,
        users: UsersService,
        client: ProviderClient,
        jwt: JwtSettings,
        require_verified_email: bool,
    ) -> Self {
        Self {
            repo,
            users,
            client,
            jwt,
            require_verified_email,
        }
    }

    pub fn providers(&self) -> Vec<ProviderInfo> {
        self.client
            .enabled()
            .into_iter()
            .map(ProviderInfo::new)
            .collect()
    }

    /// Start a sign-in or sign-up: where to send the browser.
    pub async fn login_url(&self, provider: Provider, redirect: Option<&str>) -> AppResult<String> {
        self.start(provider, None, safe_redirect_path(redirect))
            .await
    }

    /// Start linking a provider to the signed-in user's account.
    pub async fn link_url(
        &self,
        actor: &SessionUser,
        provider: Provider,
    ) -> AppResult<AuthorizeUrlResponse> {
        let authorize_url = self
            .start(provider, Some(actor.0.id), "/settings".to_string())
            .await?;
        Ok(AuthorizeUrlResponse { authorize_url })
    }

    async fn start(
        &self,
        provider: Provider,
        user_id: Option<Uuid>,
        redirect_path: String,
    ) -> AppResult<String> {
        if !self.client.is_enabled(provider) {
            return Err(OAuthError::ProviderUnavailable.into());
        }
        // `state` ties the callback to this request (CSRF protection); the PKCE verifier stays
        // on the server, so a stolen authorization code is useless without it.
        let state = secret_token::generate();
        let verifier = secret_token::generate().raw;
        let challenge = secret_token::pkce_challenge(&verifier);

        self.repo
            .insert_state(
                &state.hash,
                provider,
                &verifier,
                user_id,
                &redirect_path,
                Utc::now() + STATE_TTL,
            )
            .await?;
        Ok(self
            .client
            .authorize_url(provider, &state.raw, &challenge)?)
    }

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

    /// `POST /auth/oauth/exchange`: trades the one-time code for an access token.
    pub async fn exchange(&self, code: &str) -> AppResult<TokenResponse> {
        let user_id = self
            .repo
            .claim_grant(&secret_token::hash(code))
            .await?
            .ok_or(OAuthError::InvalidGrant)?;
        let user = self
            .users
            .find_by_id(user_id)
            .await?
            .filter(|u| u.is_active)
            .ok_or(OAuthError::AccountDisabled)?;

        let access_token = jwt::issue(
            user.id,
            user.role,
            user.token_version,
            &self.jwt.secret,
            self.jwt.ttl_secs,
        )?;
        Ok(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in: self.jwt.ttl_secs,
        })
    }

    pub async fn identities(&self, actor: &SessionUser) -> AppResult<Vec<IdentityResponse>> {
        Ok(self
            .repo
            .list_identities(actor.0.id)
            .await?
            .into_iter()
            .map(IdentityResponse::from)
            .collect())
    }

    /// Removes the link, unless it is the account's only way to sign in.
    pub async fn unlink(&self, actor: &SessionUser, provider: Provider) -> AppResult<()> {
        let linked = self
            .repo
            .list_identities(actor.0.id)
            .await?
            .into_iter()
            .any(|i| i.provider == provider);
        if !linked {
            return Err(OAuthError::NotLinked.into());
        }
        if self.users.sign_in_method_count(actor.0.id).await? < 2 {
            return Err(OAuthError::LastSignInMethod.into());
        }
        self.repo.delete_identity(actor.0.id, provider).await?;
        Ok(())
    }
}

fn server_error(error: AppError) -> &'static str {
    tracing::error!(?error, "oauth callback failed");
    "server_error"
}

fn provider_error(error: OAuthError) -> &'static str {
    tracing::warn!(error = %error, detail = ?error, "oauth provider call failed");
    "provider_error"
}

/// Only same-site relative paths are allowed, so the redirect can never leave the frontend.
pub fn safe_redirect_path(input: Option<&str>) -> String {
    match input {
        Some(path)
            if path.starts_with('/')
                && !path.starts_with("//")
                && !path.contains('\\')
                && !path.chars().any(char::is_control)
                && path.len() <= 200 =>
        {
            path.to_string()
        }
        _ => "/".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_paths_must_stay_on_the_frontend() {
        assert_eq!(
            safe_redirect_path(Some("/settings?tab=security")),
            "/settings?tab=security"
        );
        assert_eq!(safe_redirect_path(None), "/");
        for evil in [
            "https://evil.example",
            "//evil.example",
            "/\\evil.example",
            "javascript:alert(1)",
            "evil",
            "/ok\r\nSet-Cookie: x=1",
        ] {
            assert_eq!(safe_redirect_path(Some(evil)), "/", "{evil}");
        }
        assert_eq!(
            safe_redirect_path(Some(&format!("/{}", "a".repeat(300)))),
            "/"
        );
    }
}
