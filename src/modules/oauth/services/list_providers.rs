//! Which sign-in providers are enabled.

use super::OAuthService;
use crate::modules::oauth::dto::ProviderInfo;

impl OAuthService {
    pub fn providers(&self) -> Vec<ProviderInfo> {
        self.client
            .enabled()
            .into_iter()
            .map(ProviderInfo::new)
            .collect()
    }
}
