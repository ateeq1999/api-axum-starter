use std::time::Duration;

use axum::{Router, http::StatusCode};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer, trace::TraceLayer,
};

use super::{request_id, trace};

/// Wraps a router with the global layers (request id, tracing, timeout, compression, CORS, a
/// request body size cap). `cors_allowed_origins` and `max_request_body_bytes` come from
/// `Config`, passed in rather than read here so `common` does not depend on `config` (see
/// `app::build_router`, the only caller).
pub fn apply<S>(
    router: Router<S>,
    cors_allowed_origins: &[String],
    max_request_body_bytes: usize,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router.layer(
        ServiceBuilder::new()
            .layer(request_id::set())
            .layer(TraceLayer::new_for_http().make_span_with(trace::make_span))
            .layer(request_id::propagate())
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                Duration::from_secs(10),
            ))
            .layer(CompressionLayer::new())
            .layer(cors_layer(cors_allowed_origins))
            .layer(RequestBodyLimitLayer::new(max_request_body_bytes)),
    )
}

/// Restricts cross-origin browser requests to the configured origins. The literal origin `*`
/// opts back into allowing any origin (dev convenience, e.g. `CORS_ALLOWED_ORIGINS` unset and no
/// single frontend origin to default to). Methods/headers are left unrestricted: with bearer-
/// token auth (no cookies), the origin allow-list is what actually matters for CSRF/data-exfil
/// purposes, not which headers a request may carry.
fn cors_layer(allowed_origins: &[String]) -> CorsLayer {
    if allowed_origins.iter().any(|origin| origin == "*") {
        return CorsLayer::permissive();
    }
    let allowed_origin_header_values: Vec<_> = allowed_origins
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();
    CorsLayer::new()
        .allow_origin(allowed_origin_header_values)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any)
}
