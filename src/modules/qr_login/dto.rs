use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Returned to the device that wants to sign in (the one that shows the QR code).
#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedSession {
    pub id: String,
    /// Keep it on this device: it is required to poll the session and collect the token.
    /// It is not part of the QR code.
    pub poll_secret: String,
    /// Show this next to the QR code. The user must type it on the approving device.
    pub verification_code: String,
    /// What the QR code encodes: a link that opens the frontend or the phone app.
    pub qr_payload: String,
    /// The same QR code rendered as an SVG image, ready to display.
    pub qr_svg: String,
    pub expires_at: DateTime<Utc>,
    pub poll_interval_secs: u64,
}

/// `pending`, `scanned`, `approved` (with the token), `rejected`, `expired` or `consumed`.
#[derive(Debug, Serialize, ToSchema)]
pub struct PollResponse {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<i64>,
}

impl PollResponse {
    pub fn status(status: &'static str) -> Self {
        Self {
            status,
            access_token: None,
            token_type: None,
            expires_in: None,
        }
    }
}

/// Shown on the approving device so the user can tell whether the request is really theirs.
#[derive(Debug, Serialize, ToSchema)]
pub struct ScanResponse {
    pub requester_ip: Option<String>,
    pub requester_agent: Option<String>,
    pub requested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ApproveDto {
    /// The verification code shown on the device that is signing in.
    #[validate(length(equal = 4, message = "must be 4 digits"))]
    pub code: String,
}
