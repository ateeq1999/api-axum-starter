//! The signed-in phone claims the QR code.

use super::QrLoginService;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::qr_login::{dto::ScanResponse, entity::QrStatus, error::QrError};

impl QrLoginService {
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
}
