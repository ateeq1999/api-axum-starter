//! Sending a signed-out visitor to the provider to sign in or sign up.

use super::{OAuthService, safe_redirect_path};
use crate::common::error::AppResult;
use crate::modules::oauth::provider::Provider;

impl OAuthService {
    /// Start a sign-in or sign-up: where to send the browser.
    pub async fn login_url(&self, provider: Provider, redirect: Option<&str>) -> AppResult<String> {
        self.start(provider, None, safe_redirect_path(redirect))
            .await
    }
}
