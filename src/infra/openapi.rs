//! Aggregates every `#[utoipa::path]`-annotated handler and `#[derive(ToSchema)]` DTO into one
//! OpenAPI document, served as JSON plus a Swagger UI (see `app::build_router`).

use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "api-starter-axum",
        description = "REST API starter: users, auth (password, OAuth, passkeys, QR-code, TOTP), API keys, audit log."
    ),
    paths(
        crate::modules::health::controller::liveness,
        crate::modules::health::controller::readiness,
        crate::modules::auth::controllers::session::register,
        crate::modules::auth::controllers::session::login,
        crate::modules::auth::controllers::session::me,
        crate::modules::auth::controllers::password::forgot,
        crate::modules::auth::controllers::password::reset,
        crate::modules::auth::controllers::password::change,
        crate::modules::auth::controllers::password::invite,
        crate::modules::auth::controllers::email::verify,
        crate::modules::auth::controllers::email::resend,
        crate::modules::auth::controllers::email::request_change,
        crate::modules::auth::controllers::email::confirm_change,
        crate::modules::auth::controllers::totp::setup,
        crate::modules::auth::controllers::totp::enable,
        crate::modules::auth::controllers::totp::disable,
        crate::modules::auth::controllers::totp::verify,
        crate::modules::users::controller::list,
        crate::modules::users::controller::create,
        crate::modules::users::controller::me,
        crate::modules::users::controller::update_me,
        crate::modules::users::controller::get_one,
        crate::modules::users::controller::update,
        crate::modules::users::controller::remove,
        crate::modules::avatars::controller::upload,
        crate::modules::avatars::controller::remove,
        crate::modules::avatars::controller::serve,
        crate::modules::api_keys::controller::create,
        crate::modules::api_keys::controller::list,
        crate::modules::api_keys::controller::revoke,
        crate::modules::audit_log::controller::list,
        crate::modules::oauth::controller::providers,
        crate::modules::oauth::controller::login,
        crate::modules::oauth::controller::callback,
        crate::modules::oauth::controller::exchange,
        crate::modules::oauth::controller::link,
        crate::modules::oauth::controller::identities,
        crate::modules::oauth::controller::unlink,
        crate::modules::passkeys::controller::begin_registration,
        crate::modules::passkeys::controller::finish_registration,
        crate::modules::passkeys::controller::begin_login,
        crate::modules::passkeys::controller::finish_login,
        crate::modules::passkeys::controller::list,
        crate::modules::passkeys::controller::remove,
        crate::modules::qr_login::controller::create,
        crate::modules::qr_login::controller::poll,
        crate::modules::qr_login::controller::scan,
        crate::modules::qr_login::controller::approve,
        crate::modules::qr_login::controller::reject,
    ),
    components(schemas(
        crate::common::security::Role,
        crate::common::security::ApiKeyScope,
        crate::common::dto::MessageResponse,
        crate::modules::oauth::Provider,
        crate::modules::auth::dto::CredentialsDto,
        crate::modules::auth::dto::TokenResponse,
        crate::modules::auth::dto::LoginResponse,
        crate::modules::auth::dto::ForgotPasswordDto,
        crate::modules::auth::dto::ResetPasswordDto,
        crate::modules::auth::dto::ChangePasswordDto,
        crate::modules::auth::dto::ChangeEmailDto,
        crate::modules::auth::dto::VerifyEmailDto,
        crate::modules::auth::dto::InviteUserDto,
        crate::modules::auth::dto::DisableTotpDto,
        crate::modules::auth::dto::EnableTotpDto,
        crate::modules::auth::dto::VerifyTotpDto,
        crate::modules::auth::dto::TotpSetupResponse,
        crate::modules::auth::dto::TotpEnabledResponse,
        crate::modules::users::dto::UserResponse,
        crate::modules::users::dto::CreateUserDto,
        crate::modules::users::dto::UpdateUserDto,
        crate::modules::users::dto::UpdateProfileDto,
        crate::common::dto::PaginatedResponse<crate::modules::users::dto::UserResponse>,
        crate::modules::api_keys::dto::ApiKeyResponse,
        crate::modules::api_keys::dto::CreatedApiKeyResponse,
        crate::modules::api_keys::dto::CreateApiKeyDto,
        crate::modules::audit_log::dto::AuditLogEntryResponse,
        crate::common::dto::PaginatedResponse<crate::modules::audit_log::dto::AuditLogEntryResponse>,
        crate::modules::oauth::dto::ProviderInfo,
        crate::modules::oauth::dto::IdentityResponse,
        crate::modules::oauth::dto::AuthorizeUrlResponse,
        crate::modules::oauth::dto::ExchangeDto,
        crate::modules::passkeys::dto::PasskeyResponse,
        crate::modules::qr_login::dto::CreatedSession,
        crate::modules::qr_login::dto::PollResponse,
        crate::modules::qr_login::dto::ScanResponse,
        crate::modules::qr_login::dto::ApproveDto,
    )),
    tags(
        (name = "health", description = "Liveness/readiness checks"),
        (name = "auth", description = "Registration, password login, password reset/change, email verification, two-factor (TOTP)"),
        (name = "oauth", description = "Google/GitHub sign-in"),
        (name = "passkeys", description = "WebAuthn passkey registration and sign-in"),
        (name = "qr-login", description = "WhatsApp-style QR-code sign-in"),
        (name = "users", description = "User profiles and admin user management"),
        (name = "avatars", description = "Profile photo upload/serving"),
        (name = "api-keys", description = "Long-lived API keys"),
        (name = "audit-log", description = "Admin-readable log of administrative actions"),
    ),
    modifiers(&BearerAuthAddon)
)]
pub struct ApiDoc;

/// Registers the `bearer_auth` security scheme referenced by every `security(("bearer_auth" =
/// []))` annotation: `Authorization: Bearer <JWT or API key>`.
struct BearerAuthAddon;

impl Modify for BearerAuthAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let Some(components) = openapi.components.as_mut() else {
            return;
        };
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .description(Some(
                        "A session JWT (from /auth/login or /auth/2fa/verify) or an API key (ak_...)",
                    ))
                    .build(),
            ),
        );
    }
}
