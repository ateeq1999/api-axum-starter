use serde::Serialize;
use utoipa::ToSchema;

use super::TokenResponse;

/// What `POST /auth/login` returns: either a real session (`TokenResponse`'s shape) or, for a
/// two-factor-enabled account, a marker plus the short-lived token `POST /auth/2fa/verify` needs.
/// `#[serde(untagged)]` means each variant serializes as its own plain object — the client tells
/// them apart by checking for `access_token` vs `requires_totp`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(untagged)]
pub enum LoginResponse {
    Authenticated(TokenResponse),
    TwoFactorRequired {
        requires_totp: bool,
        pending_token: String,
    },
}

impl LoginResponse {
    pub fn two_factor_required(pending_token: String) -> Self {
        Self::TwoFactorRequired {
            requires_totp: true,
            pending_token,
        }
    }
}
