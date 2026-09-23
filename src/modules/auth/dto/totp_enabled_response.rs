use serde::Serialize;
use utoipa::ToSchema;

/// Shown exactly once, right after enabling two-factor: only hashes of these are ever stored.
#[derive(Debug, Serialize, ToSchema)]
pub struct TotpEnabledResponse {
    pub recovery_codes: Vec<String>,
}
