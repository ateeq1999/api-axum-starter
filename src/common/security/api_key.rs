use std::{future::Future, pin::Pin, sync::Arc};

use serde::{Deserialize, Serialize};

use super::auth_user::AuthUser;
use crate::common::error::AppResult;

/// Every API key starts with this, which is how a bearer credential is told apart from a JWT.
pub const API_KEY_PREFIX: &str = "ak_";

/// What an API key may do. `Read` keys are limited to safe HTTP methods (GET, HEAD, OPTIONS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum ApiKeyScope {
    #[default]
    Read,
    Write,
}

/// How the caller proved who they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Credential {
    /// A JWT from an interactive sign-in (password, OAuth, passkey, QR code).
    Session,
    /// A long-lived API key.
    ApiKey(ApiKeyScope),
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Resolves a presented API key to the user it belongs to. Implemented by the `api_keys`
/// module; `common` only knows this interface, so it does not depend on any feature.
pub trait ApiKeyVerifier: Send + Sync + 'static {
    fn verify<'a>(&'a self, presented: &'a str) -> BoxFuture<'a, AppResult<AuthUser>>;
}

/// Cheap-to-clone handle stored in the application state.
#[derive(Clone)]
pub struct ApiKeyAuth(pub Arc<dyn ApiKeyVerifier>);
