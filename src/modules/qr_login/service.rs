use chrono::{Duration, Utc};
use qrcode::{QrCode, render::svg};

use super::{
    dto::{ApproveDto, CreatedSession, PollResponse, ScanResponse},
    entity::{QrSession, QrStatus},
    error::QrError,
    repository::QrRepository,
};
use crate::{
    common::{
        error::{AppError, AppResult},
        extractors::ClientMeta,
        security::{JwtSettings, SessionUser, jwt, secret_token},
    },
    config::QrLoginConfig,
    modules::users::UsersService,
};

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

    /// Step 1, from the device that wants to sign in. No authentication needed.
    pub async fn create(&self, meta: ClientMeta) -> AppResult<CreatedSession> {
        let id: String = secret_token::generate().raw.chars().take(22).collect();
        let secret = secret_token::generate();
        let code = format!("{:04}", rand::random_range(0..10_000u32));
        let expires_at = Utc::now() + self.ttl;

        self.repo
            .insert(
                &id,
                &secret.hash,
                &code,
                meta.ip.as_deref(),
                meta.user_agent.as_deref(),
                expires_at,
            )
            .await?;

        let qr_payload = format!("{}/qr-login?session={id}", self.frontend_url);
        let qr_svg = QrCode::new(qr_payload.as_bytes())
            .map_err(|e| AppError::Internal(anyhow::anyhow!("qr encoding failed: {e}")))?
            .render::<svg::Color>()
            .min_dimensions(256, 256)
            .quiet_zone(true)
            .build();

        Ok(CreatedSession {
            id,
            poll_secret: secret.raw,
            verification_code: code,
            qr_payload,
            qr_svg,
            expires_at,
            poll_interval_secs: POLL_INTERVAL_SECS,
        })
    }

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

    /// Step 2, from the signed-in device that scanned the QR code.
    pub async fn scan(&self, actor: &SessionUser, id: &str) -> AppResult<ScanResponse> {
        let session = self.find_live(id).await?;

        match (session.status, session.user_id) {
            (QrStatus::Pending, _) => {
                if !self.repo.mark_scanned(id, actor.0.id).await? {
                    // Lost a race with another scanner (or it just expired): decide again.
                    let now = self.find_live(id).await?;
                    if now.user_id != Some(actor.0.id) {
                        return Err(QrError::AlreadyScanned.into());
                    }
                }
            }
            (QrStatus::Scanned, Some(scanner)) if scanner == actor.0.id => {}
            (QrStatus::Scanned, _) => return Err(QrError::AlreadyScanned.into()),
            _ => return Err(QrError::NotFound.into()),
        }

        Ok(ScanResponse {
            requester_ip: session.requester_ip,
            requester_agent: session.requester_agent,
            requested_at: session.created_at,
            expires_at: session.expires_at,
        })
    }

    /// Step 3: the user confirms, proving they can see the other device's screen.
    pub async fn approve(&self, actor: &SessionUser, id: &str, dto: ApproveDto) -> AppResult<()> {
        let session = self.find_live(id).await?;
        self.require_scanned_by(&session, actor)?;

        if session.code != dto.code {
            let cancelled = self.repo.record_wrong_code(id).await?;
            metrics::counter!("qr_login_total", "outcome" => "wrong_code").increment(1);
            return Err(if cancelled {
                QrError::TooManyAttempts
            } else {
                QrError::WrongCode
            }
            .into());
        }
        if !self
            .repo
            .resolve(id, actor.0.id, QrStatus::Approved)
            .await?
        {
            return Err(QrError::NotFound.into());
        }
        Ok(())
    }

    pub async fn reject(&self, actor: &SessionUser, id: &str) -> AppResult<()> {
        let session = self.find_live(id).await?;
        self.require_scanned_by(&session, actor)?;
        self.repo
            .resolve(id, actor.0.id, QrStatus::Rejected)
            .await?;
        Ok(())
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
