//! The new device polls until the phone approves or rejects.

use super::QrLoginService;
use crate::common::{
    error::AppResult,
    security::{jwt, secret_token},
};
use crate::modules::qr_login::{dto::PollResponse, entity::QrStatus, error::QrError};
use chrono::Utc;

impl QrLoginService {
    /// Step 4, polled by the new device with its secret. Returns the token once, on approval.
    pub async fn poll(&self, id: &str, secret: Option<&str>) -> AppResult<PollResponse> {
        let session = self.find(id).await?;
        // Constant-time comparison is unnecessary: the values compared are SHA-256 hashes.
        if secret.map(secret_token::hash).as_deref() != Some(session.secret_hash.as_str()) {
            return Err(QrError::NotFound.into());
        }

        match session.status {
            QrStatus::Consumed => Ok(PollResponse::status("consumed")),
            QrStatus::Rejected => Ok(PollResponse::status("rejected")),
            _ if session.expires_at <= Utc::now() => Ok(PollResponse::status("expired")),
            QrStatus::Approved => self.collect_token(id).await,
            other => Ok(PollResponse::status(other.as_str())),
        }
    }

    async fn collect_token(&self, id: &str) -> AppResult<PollResponse> {
        let Some(user_id) = self.repo.consume(id).await? else {
            // Another poll won the race; the token is gone.
            return Ok(PollResponse::status("consumed"));
        };
        let user = self
            .users
            .find_by_id(user_id)
            .await?
            .filter(|u| u.is_active)
            .ok_or(QrError::NotFound)?;
        if self.require_verified_email && !user.is_email_verified() {
            return Err(QrError::EmailNotVerified.into());
        }

        let access_token = jwt::issue(
            user.id,
            user.role,
            user.token_version,
            &self.jwt.secret,
            self.jwt.ttl_secs,
        )?;
        metrics::counter!("qr_login_total", "outcome" => "success").increment(1);
        Ok(PollResponse {
            status: "approved",
            access_token: Some(access_token),
            token_type: Some("Bearer"),
            expires_in: Some(self.jwt.ttl_secs),
        })
    }
}
