//! The live account check behind every authenticated request.

use uuid::Uuid;

use super::UsersService;
use crate::common::{
    error::AppResult,
    security::{AuthUser, SecurityError, SessionVerifier, api_key::BoxFuture},
};

impl SessionVerifier for UsersService {
    fn verify<'a>(
        &'a self,
        user_id: Uuid,
        token_version: i32,
    ) -> BoxFuture<'a, AppResult<AuthUser>> {
        Box::pin(async move {
            // `find_by_id` already filters out soft-deleted rows.
            let user = self
                .find_by_id(user_id)
                .await?
                .filter(|u| u.is_active)
                .ok_or(SecurityError::InvalidToken)?;
            if user.token_version != token_version {
                return Err(SecurityError::InvalidToken.into());
            }
            Ok(AuthUser::session(user.id, user.role))
        })
    }
}
