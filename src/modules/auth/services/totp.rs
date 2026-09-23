use super::one_time_tokens::OneTimeTokenService;
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, JwtSettings, jwt, password, secret_token, totp},
    },
    modules::{
        auth::{
            dto::{TokenResponse, TotpEnabledResponse, TotpSetupResponse},
            entity::TokenPurpose,
            error::AuthError,
            helpers::recovery_code,
            repositories::TotpRecoveryCodeRepository,
        },
        users::{User, UsersError, UsersService},
    },
};

/// TOTP-based two-factor authentication: enrollment (`begin_setup`/`confirm_setup`), turning it
/// back off (`disable`), and completing a login that a correct password alone was not enough for
/// (`verify_login`). The password check itself and issuing the pending token stay in
/// `SessionService::login`; this only owns what happens with/to the second factor.
#[derive(Clone)]
pub struct TotpService {
    users: UsersService,
    tokens: OneTimeTokenService,
    recovery_codes: TotpRecoveryCodeRepository,
    jwt: JwtSettings,
    /// Shown as the "issuer" in the authenticator app (next to the account email).
    issuer: String,
}

impl TotpService {
    pub fn new(
        users: UsersService,
        tokens: OneTimeTokenService,
        recovery_codes: TotpRecoveryCodeRepository,
        jwt: JwtSettings,
        issuer: String,
    ) -> Self {
        Self {
            users,
            tokens,
            recovery_codes,
            jwt,
            issuer,
        }
    }

    /// Starts (or restarts) enrollment: generates a new secret and stores it, but two-factor is
    /// not enforced at login until [`Self::confirm_setup`] proves the app has it too.
    pub async fn begin_setup(&self, actor: &AuthUser) -> AppResult<TotpSetupResponse> {
        let user = self.require_active_user(actor.id).await?;
        let generated = totp::generate_authenticator_secret(&self.issuer, &user.email)?;
        self.users
            .set_pending_totp_secret(user.id, &generated.base32_secret)
            .await?;
        Ok(TotpSetupResponse {
            secret: generated.base32_secret,
            qr_code_data_uri: generated.qr_code_data_uri,
            provisioning_uri: generated.provisioning_uri,
        })
    }

    /// Confirms enrollment: the code must come from the secret [`Self::begin_setup`] just issued.
    /// Turns two-factor on and returns a fresh batch of recovery codes, shown to the caller
    /// exactly once.
    pub async fn confirm_setup(
        &self,
        actor: &AuthUser,
        code: &str,
    ) -> AppResult<TotpEnabledResponse> {
        let user = self.require_active_user(actor.id).await?;
        if user.totp_enabled {
            return Err(AuthError::TotpAlreadyEnabled.into());
        }
        let pending_secret = user
            .totp_secret
            .as_deref()
            .ok_or(AuthError::NoTotpSetupInProgress)?;
        if !totp::verify_code(pending_secret, &self.issuer, &user.email, code) {
            return Err(AuthError::InvalidTotpCode.into());
        }

        self.users.enable_totp(user.id).await?;
        let batch = recovery_code::generate_batch();
        let hashes: Vec<String> = batch.iter().map(|c| c.hash.clone()).collect();
        self.recovery_codes.replace_all(user.id, &hashes).await?;

        Ok(TotpEnabledResponse {
            recovery_codes: batch.into_iter().map(|c| c.raw).collect(),
        })
    }

    /// Requires the current password so a stolen session token alone cannot turn two-factor off.
    pub async fn disable(&self, actor: &AuthUser, current_password: String) -> AppResult<()> {
        let user = self.require_active_user(actor.id).await?;
        let password_ok =
            password::verify_blocking(current_password, user.password_hash.clone()).await?;
        if !password_ok {
            return Err(AuthError::WrongCurrentPassword.into());
        }
        self.users.disable_totp(user.id).await?;
        self.recovery_codes.delete_all(user.id).await?;
        Ok(())
    }

    /// Completes a login that a correct password alone was not enough for. `code` may be a live
    /// authenticator code or an unused recovery code.
    pub async fn verify_login(&self, pending_token: &str, code: &str) -> AppResult<TokenResponse> {
        let failed = || AuthError::InvalidTotpCode;

        // Peeked, not consumed yet: a mistyped code must not burn the whole pending login, so the
        // same pending_token stays usable for another attempt within its TTL. Only a successful
        // code (below) actually spends it.
        let claim = self
            .tokens
            .peek(TokenPurpose::TwoFactorPending, pending_token)
            .await?;
        let user = self
            .users
            .find_by_id(claim.user_id)
            .await?
            .filter(|u| u.is_active && u.totp_enabled)
            .ok_or_else(failed)?;
        let secret = user.totp_secret.as_deref().ok_or_else(failed)?;

        let code_is_valid = totp::verify_code(secret, &self.issuer, &user.email, code)
            || self
                .recovery_codes
                .claim(user.id, &secret_token::hash(code))
                .await?;
        if !code_is_valid {
            metrics::counter!("auth_totp_verify_total", "outcome" => "failure").increment(1);
            return Err(failed().into());
        }
        self.tokens
            .redeem(TokenPurpose::TwoFactorPending, pending_token)
            .await?;

        let access_token = jwt::issue(
            user.id,
            user.role,
            user.token_version,
            &self.jwt.secret,
            self.jwt.ttl_secs,
        )?;
        metrics::counter!("auth_totp_verify_total", "outcome" => "success").increment(1);
        Ok(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in: self.jwt.ttl_secs,
        })
    }

    async fn require_active_user(&self, id: uuid::Uuid) -> AppResult<User> {
        Ok(self
            .users
            .find_by_id(id)
            .await?
            .filter(|u| u.is_active)
            .ok_or(UsersError::NotFound)?)
    }
}
