//! A new device asks for a sign-in QR code.

use super::{POLL_INTERVAL_SECS, QrLoginService};
use crate::common::{
    error::{AppError, AppResult},
    extractors::ClientMeta,
    security::secret_token,
};
use crate::modules::qr_login::dto::CreatedSession;
use chrono::Utc;
use qrcode::{QrCode, render::svg};

impl QrLoginService {
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
}
