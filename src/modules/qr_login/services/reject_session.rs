//! The phone refuses the sign-in.

use super::QrLoginService;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::qr_login::entity::QrStatus;

impl QrLoginService {
    pub async fn reject(&self, actor: &SessionUser, id: &str) -> AppResult<()> {
        let session = self.find_live(id).await?;
        self.require_scanned_by(&session, actor)?;
        self.repo
            .resolve(id, actor.0.id, QrStatus::Rejected)
            .await?;
        Ok(())
    }
}
