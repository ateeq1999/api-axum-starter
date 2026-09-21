pub mod email;
pub mod password;
pub mod session;

use axum::{Router, middleware};

use crate::{
    common::middleware::rate_limit::{self, RateLimiter},
    state::AppState,
};

pub fn router(limiter: RateLimiter) -> Router<AppState> {
    let limited = Router::new()
        .merge(session::rate_limited())
        .merge(password::rate_limited())
        .merge(email::rate_limited())
        .route_layer(middleware::from_fn_with_state(limiter, rate_limit::enforce));

    limited
        .merge(session::authenticated())
        .merge(password::authenticated())
        .merge(email::authenticated())
}
