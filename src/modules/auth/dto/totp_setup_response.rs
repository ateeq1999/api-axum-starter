use serde::Serialize;
use utoipa::ToSchema;

/// Everything needed to enroll an authenticator app. Two-factor is not yet active — the caller
/// must confirm a code from it via `POST /auth/2fa/enable` first.
#[derive(Debug, Serialize, ToSchema)]
pub struct TotpSetupResponse {
    /// Base32-encoded; lets the user type the secret in by hand if they cannot scan the QR code.
    pub secret: String,
    pub qr_code_data_uri: String,
    pub provisioning_uri: String,
}
