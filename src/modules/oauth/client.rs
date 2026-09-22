//! The two HTTP calls of the authorization-code flow, plus profile lookup, for Google and GitHub.

use std::{sync::Arc, time::Duration};

use serde_json::Value;

use super::{entity::ProviderProfile, error::OAuthError, provider::Provider};
use crate::config::{OAuthConfig, OAuthProviderConfig};

#[derive(Clone)]
pub struct ProviderClient {
    http: reqwest::Client,
    public_api_url: Arc<str>,
    google: Option<Arc<OAuthProviderConfig>>,
    github: Option<Arc<OAuthProviderConfig>>,
}

impl ProviderClient {
    pub fn new(config: &OAuthConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("api-starter-axum")
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            // Never follow redirects while talking to a provider.
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http,
            public_api_url: config.public_api_url.as_str().into(),
            google: config.google.clone().map(Arc::new),
            github: config.github.clone().map(Arc::new),
        })
    }

    fn settings(&self, provider: Provider) -> Option<&OAuthProviderConfig> {
        match provider {
            Provider::Google => self.google.as_deref(),
            Provider::Github => self.github.as_deref(),
        }
    }

    pub fn is_enabled(&self, provider: Provider) -> bool {
        self.settings(provider).is_some()
    }

    pub fn enabled(&self) -> Vec<Provider> {
        Provider::ALL
            .into_iter()
            .filter(|p| self.is_enabled(*p))
            .collect()
    }

    /// The URL to register at the provider as the authorized redirect / callback URL.
    pub fn redirect_uri(&self, provider: Provider) -> String {
        format!(
            "{}/api/v1/auth/oauth/{}/callback",
            self.public_api_url,
            provider.as_str()
        )
    }

    /// Where to send the browser to start the sign-in at the provider.
    pub fn authorize_url(
        &self,
        provider: Provider,
        state: &str,
        code_challenge: &str,
    ) -> Result<String, OAuthError> {
        let settings = self
            .settings(provider)
            .ok_or(OAuthError::ProviderUnavailable)?;
        let mut url = url::Url::parse(&settings.authorize_url)
            .map_err(|e| OAuthError::Provider(format!("bad authorize URL: {e}")))?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("response_type", "code")
                .append_pair("client_id", &settings.client_id)
                .append_pair("redirect_uri", &self.redirect_uri(provider))
                .append_pair("scope", provider.scopes())
                .append_pair("state", state)
                .append_pair("code_challenge", code_challenge)
                .append_pair("code_challenge_method", "S256");
            match provider {
                Provider::Google => {
                    query.append_pair("prompt", "select_account");
                }
                Provider::Github => {
                    query.append_pair("allow_signup", "true");
                }
            }
        }
        Ok(url.into())
    }

    /// Trades the one-time `code` for an access token (proving possession of the PKCE verifier).
    pub async fn exchange_code(
        &self,
        provider: Provider,
        code: &str,
        code_verifier: &str,
    ) -> Result<String, OAuthError> {
        let settings = self
            .settings(provider)
            .ok_or(OAuthError::ProviderUnavailable)?;
        let response = self
            .http
            .post(&settings.token_url)
            .header("Accept", "application/json")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &self.redirect_uri(provider)),
                ("client_id", &settings.client_id),
                ("client_secret", &settings.client_secret),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await
            .map_err(|e| OAuthError::Provider(format!("token request failed: {e}")))?;

        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|e| OAuthError::Provider(format!("token response is not JSON: {e}")))?;
        // GitHub reports errors with HTTP 200 and an `error` field.
        match body.get("access_token").and_then(Value::as_str) {
            Some(token) if status.is_success() => Ok(token.to_string()),
            _ => Err(OAuthError::Provider(format!(
                "token exchange rejected ({status}): {}",
                body.get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("no access_token")
            ))),
        }
    }

    pub async fn fetch_profile(
        &self,
        provider: Provider,
        access_token: &str,
    ) -> Result<ProviderProfile, OAuthError> {
        match provider {
            Provider::Google => self.google_profile(access_token).await,
            Provider::Github => self.github_profile(access_token).await,
        }
    }

    async fn get_json(&self, url: &str, access_token: &str) -> Result<Value, OAuthError> {
        let response = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .header("Accept", "application/vnd.github+json, application/json")
            .send()
            .await
            .map_err(|e| OAuthError::Provider(format!("profile request failed: {e}")))?;
        if !response.status().is_success() {
            return Err(OAuthError::Provider(format!(
                "profile request returned {}",
                response.status()
            )));
        }
        response
            .json()
            .await
            .map_err(|e| OAuthError::Provider(format!("profile response is not JSON: {e}")))
    }

    async fn google_profile(&self, access_token: &str) -> Result<ProviderProfile, OAuthError> {
        let settings = self
            .settings(Provider::Google)
            .ok_or(OAuthError::ProviderUnavailable)?;
        let body = self.get_json(&settings.userinfo_url, access_token).await?;
        parse_google_profile(&body)
    }

    async fn github_profile(&self, access_token: &str) -> Result<ProviderProfile, OAuthError> {
        let settings = self
            .settings(Provider::Github)
            .ok_or(OAuthError::ProviderUnavailable)?;
        let user = self.get_json(&settings.userinfo_url, access_token).await?;

        // The profile's email may be private or unverified; the emails list says which are verified.
        let emails = match &settings.emails_url {
            Some(url) => self.get_json(url, access_token).await.ok(),
            None => None,
        };
        parse_github_profile(&user, emails.as_ref())
    }
}

fn parse_google_profile(body: &Value) -> Result<ProviderProfile, OAuthError> {
    let sub = body
        .get("sub")
        .and_then(Value::as_str)
        .ok_or_else(|| OAuthError::Provider("google profile has no `sub`".into()))?;
    let verified = match body.get("email_verified") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true",
        _ => false,
    };
    Ok(ProviderProfile {
        provider_user_id: sub.to_string(),
        email: body
            .get("email")
            .and_then(Value::as_str)
            .map(str::to_string),
        email_verified: verified,
        name: body.get("name").and_then(Value::as_str).map(str::to_string),
    })
}

fn parse_github_profile(
    user: &Value,
    emails: Option<&Value>,
) -> Result<ProviderProfile, OAuthError> {
    let id = user
        .get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| OAuthError::Provider("github profile has no `id`".into()))?;
    let name = user
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| user.get("login").and_then(Value::as_str))
        .map(str::to_string);

    let verified_email = emails.and_then(Value::as_array).and_then(|list| {
        let entry = |e: &&Value| e.get("verified").and_then(Value::as_bool) == Some(true);
        let primary = |e: &&Value| e.get("primary").and_then(Value::as_bool) == Some(true);
        list.iter()
            .filter(entry)
            .find(primary)
            .or_else(|| list.iter().find(entry))
            .and_then(|e| e.get("email").and_then(Value::as_str))
            .map(str::to_string)
    });

    Ok(ProviderProfile {
        provider_user_id: id.to_string(),
        email_verified: verified_email.is_some(),
        email: verified_email,
        name,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn google_profile_reads_verification_flag() {
        let p = parse_google_profile(&json!({
            "sub": "123", "email": "a@b.com", "email_verified": true, "name": "A"
        }))
        .unwrap();
        assert_eq!(
            (p.provider_user_id.as_str(), p.email_verified),
            ("123", true)
        );

        let stringly =
            parse_google_profile(&json!({"sub":"1","email":"a@b.com","email_verified":"true"}))
                .unwrap();
        assert!(stringly.email_verified);

        let missing = parse_google_profile(&json!({"sub":"1","email":"a@b.com"})).unwrap();
        assert!(!missing.email_verified);
        assert!(parse_google_profile(&json!({"email":"a@b.com"})).is_err());
    }

    #[test]
    fn github_uses_the_primary_verified_email() {
        let user = json!({"id": 42, "login": "octo", "name": null, "email": "public@x.com"});
        let emails = json!([
            {"email": "old@x.com", "primary": false, "verified": true},
            {"email": "main@x.com", "primary": true, "verified": true},
            {"email": "unverified@x.com", "primary": false, "verified": false}
        ]);
        let p = parse_github_profile(&user, Some(&emails)).unwrap();
        assert_eq!(p.provider_user_id, "42");
        assert_eq!(p.email.as_deref(), Some("main@x.com"));
        assert!(p.email_verified);
        assert_eq!(p.name.as_deref(), Some("octo"));
    }

    #[test]
    fn github_without_a_verified_email_is_unverified() {
        let user = json!({"id": 1, "login": "octo", "email": "public@x.com"});
        let none_verified = json!([{"email": "a@x.com", "primary": true, "verified": false}]);
        let p = parse_github_profile(&user, Some(&none_verified)).unwrap();
        assert!(!p.email_verified);
        assert!(p.email.is_none());
        assert!(!parse_github_profile(&user, None).unwrap().email_verified);
    }

    #[test]
    fn authorize_url_carries_pkce_and_state() {
        let client = ProviderClient::new(&OAuthConfig {
            public_api_url: "http://localhost:3000".into(),
            google: Some(OAuthProviderConfig::google("cid".into(), "secret".into())),
            github: None,
        })
        .unwrap();
        let url = client
            .authorize_url(Provider::Google, "STATE", "CHALLENGE")
            .unwrap();
        for expected in [
            "client_id=cid",
            "state=STATE",
            "code_challenge=CHALLENGE",
            "code_challenge_method=S256",
            "redirect_uri=http%3A%2F%2Flocalhost%3A3000%2Fapi%2Fv1%2Fauth%2Foauth%2Fgoogle%2Fcallback",
        ] {
            assert!(url.contains(expected), "{expected} missing from {url}");
        }
        assert!(!url.contains("secret"));
        assert!(client.authorize_url(Provider::Github, "s", "c").is_err());
    }
}
