use chrono::{Duration, Utc};

use super::{
    entity::{QrSession, QrStatus},
    error::QrError,
    repository::QrRepository,
};
use crate::{
    common::{
        error::AppResult,
        security::{JwtSettings, SessionUser},
    },
    config::QrLoginConfig,
    modules::users::UsersService,
};

mod approve_session;
mod create_session;
mod poll_session;
mod reject_session;
mod scan_session;

const POLL_INTERVAL_SECS: u64 = 2;

/// "Sign in with a QR code", WhatsApp Web style.
///
/// 1. The new device (browser) creates a session and shows its QR code and a 4-digit code.
/// 2. A device that is already signed in (the phone app) scans the QR code and calls `scan`.
/// 3. The phone shows where the request came from; the user types the 4-digit code and approves.
/// 4. The new device, which polls the session, receives an access token exactly once.
#[derive(Clone)]
pub struct QrLoginService {
    repo: QrRepository,
    users: UsersService,
    jwt: JwtSettings,
    frontend_url: String,
    ttl: Duration,
    require_verified_email: bool,
}

impl QrLoginService {
    pub fn new(
        config: &QrLoginConfig,
        frontend_url: &str,
        repo: QrRepository,
        users: UsersService,
        jwt: JwtSettings,
        require_verified_email: bool,
    ) -> Self {
        Self {
            repo,
            users,
            jwt,
            frontend_url: frontend_url.trim_end_matches('/').to_string(),
            ttl: Duration::seconds(config.ttl_secs),
            require_verified_email,
        }
    }

    fn require_scanned_by(&self, session: &QrSession, actor: &SessionUser) -> AppResult<()> {
        if session.status == QrStatus::Scanned && session.user_id == Some(actor.0.id) {
            Ok(())
        } else {
            Err(QrError::NotFound.into())
        }
    }

    async fn find(&self, id: &str) -> AppResult<QrSession> {
        Ok(self.repo.find(id).await?.ok_or(QrError::NotFound)?)
    }

    /// Like `find`, but an expired session is reported as such.
    async fn find_live(&self, id: &str) -> AppResult<QrSession> {
        let session = self.find(id).await?;
        if session.expires_at <= Utc::now() {
            return Err(QrError::Expired.into());
        }
        Ok(session)
    }
}
