//! Auth endpoints, one file per endpoint. The routes are listed here so the rate-limited
//! (pre-sign-in) set and the authenticated set are visible in one place.

pub mod change_password;
pub mod confirm_email_change;
pub mod current_user;
pub mod forgot_password;
pub mod invite_user;
pub mod login;
pub mod register;
pub mod request_email_change;
pub mod resend_verification;
pub mod reset_password;
pub mod two_factor_disable;
pub mod two_factor_enable;
pub mod two_factor_setup;
pub mod two_factor_verify;
pub mod verify_email;

use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::{
    common::middleware::rate_limit::{self, RateLimiter},
    state::AppState,
};

pub fn router(limiter: RateLimiter) -> Router<AppState> {
    // Public endpoints an attacker could hammer: limited per client IP.
    let rate_limited = Router::new()
        .route("/register", post(register::register))
        .route("/login", post(login::login))
        .route("/password/forgot", post(forgot_password::forgot))
        .route("/password/reset", post(reset_password::reset))
        .route("/email/verify", post(verify_email::verify))
        .route(
            "/email/verification/resend",
            post(resend_verification::resend),
        )
        .route(
            "/email/change/confirm",
            post(confirm_email_change::confirm_change),
        )
        .route("/2fa/verify", post(two_factor_verify::verify))
        .route_layer(middleware::from_fn_with_state(limiter, rate_limit::enforce));

    // Everything else needs a signed-in caller (the guard is in each handler's signature).
    let authenticated = Router::new()
        .route("/me", get(current_user::me))
        .route("/password/change", post(change_password::change))
        .route("/invitations", post(invite_user::invite))
        .route("/email/change", post(request_email_change::request_change))
        .route("/2fa/setup", post(two_factor_setup::setup))
        .route("/2fa/enable", post(two_factor_enable::enable))
        .route("/2fa/disable", post(two_factor_disable::disable));

    rate_limited.merge(authenticated)
}
