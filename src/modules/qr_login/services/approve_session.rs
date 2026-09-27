//! The phone approves the sign-in with the code shown on the new device.

use super::QrLoginService;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::qr_login::{dto::ApproveDto, entity::QrStatus, error::QrError};

impl QrLoginService {
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
}
