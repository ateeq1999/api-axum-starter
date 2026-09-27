use chrono::{Duration, Utc};
use uuid::Uuid;

use super::{
    client::ProviderClient, error::OAuthError, provider::Provider, repositories::OAuthRepository,
};
use crate::{
    common::{
        error::{AppError, AppResult},
        security::{JwtSettings, secret_token},
    },
    modules::users::UsersService,
};

mod exchange_code;
mod handle_callback;
mod list_identities;
mod list_providers;
mod start_linking;
mod start_sign_in;
mod unlink_provider;

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
