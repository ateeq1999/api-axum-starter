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
        crate::modules::auth::controllers::register::register,
        crate::modules::auth::controllers::login::login,
        crate::modules::auth::controllers::current_user::me,
        crate::modules::auth::controllers::forgot_password::forgot,
        crate::modules::auth::controllers::reset_password::reset,
        crate::modules::auth::controllers::change_password::change,
        crate::modules::auth::controllers::invite_user::invite,
        crate::modules::auth::controllers::verify_email::verify,
        crate::modules::auth::controllers::resend_verification::resend,
        crate::modules::auth::controllers::request_email_change::request_change,
        crate::modules::auth::controllers::confirm_email_change::confirm_change,
        crate::modules::auth::controllers::two_factor_setup::setup,
        crate::modules::auth::controllers::two_factor_enable::enable,
        crate::modules::auth::controllers::two_factor_disable::disable,
        crate::modules::auth::controllers::two_factor_verify::verify,
        crate::modules::users::controllers::list_users::list,
        crate::modules::users::controllers::create_user::create,
        crate::modules::users::controllers::get_current_user::me,
        crate::modules::users::controllers::update_profile::update_me,
        crate::modules::users::controllers::get_user::get_one,
        crate::modules::users::controllers::update_user::update,
        crate::modules::users::controllers::delete_user::remove,
        crate::modules::avatars::controllers::set_avatar::upload,
        crate::modules::avatars::controllers::remove_avatar::remove,
        crate::modules::avatars::controllers::serve_avatar::serve,
        crate::modules::media::controllers::upload_media::upload,
        crate::modules::media::controllers::list_media::list,
        crate::modules::media::controllers::get_metadata::metadata,
        crate::modules::media::controllers::download_content::content,
        crate::modules::media::controllers::delete_media::remove,
        crate::modules::api_keys::controllers::create_key::create,
        crate::modules::api_keys::controllers::list_keys::list,
        crate::modules::api_keys::controllers::revoke_key::revoke,
        crate::modules::audit_log::controller::list,
        crate::modules::oauth::controllers::list_providers::providers,
        crate::modules::oauth::controllers::start_sign_in::login,
        crate::modules::oauth::controllers::handle_callback::callback,
        crate::modules::oauth::controllers::exchange_code::exchange,
        crate::modules::oauth::controllers::start_linking::link,
        crate::modules::oauth::controllers::list_identities::identities,
        crate::modules::oauth::controllers::unlink_provider::unlink,
        crate::modules::passkeys::controllers::register_passkey::begin_registration,
        crate::modules::passkeys::controllers::register_passkey::finish_registration,
        crate::modules::passkeys::controllers::sign_in_with_passkey::begin_login,
        crate::modules::passkeys::controllers::sign_in_with_passkey::finish_login,
        crate::modules::passkeys::controllers::list_passkeys::list,
        crate::modules::passkeys::controllers::delete_passkey::remove,
        crate::modules::qr_login::controllers::create_session::create,
        crate::modules::qr_login::controllers::poll_session::poll,
        crate::modules::qr_login::controllers::scan_session::scan,
        crate::modules::qr_login::controllers::approve_session::approve,
        crate::modules::qr_login::controllers::reject_session::reject,
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
        crate::modules::media::dto::MediaResponse,
        crate::common::dto::PaginatedResponse<crate::modules::media::dto::MediaResponse>,
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
        (name = "media", description = "User file uploads (images, documents, audio/video) in S3 or local storage"),
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

/// The complete OpenAPI document served at `/api-docs/openapi.json`.
pub fn document() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
