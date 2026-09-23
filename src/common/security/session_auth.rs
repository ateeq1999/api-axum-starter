use std::sync::Arc;

use uuid::Uuid;

use super::{api_key::BoxFuture, auth_user::AuthUser};
use crate::common::error::AppResult;

/// Resolves a JWT's `(user id, token_version)` to the current [`AuthUser`] — reading the live
/// role (so a role change takes effect immediately, not just at next login) and rejecting the
/// token if the account is inactive/deleted or `token_version` no longer matches (the password
/// changed since this token was issued). Implemented by the `users` module; `common` only knows
/// this interface, so it does not depend on any feature (same pattern as [`super::ApiKeyAuth`]).
pub trait SessionVerifier: Send + Sync + 'static {
    fn verify<'a>(
        &'a self,
        user_id: Uuid,
        token_version: i32,
    ) -> BoxFuture<'a, AppResult<AuthUser>>;
}

/// Cheap-to-clone handle stored in the application state.
#[derive(Clone)]
pub struct SessionAuth(pub Arc<dyn SessionVerifier>);
