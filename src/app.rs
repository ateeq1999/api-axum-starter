use std::time::Duration;

use axum::{
    Router,
    http::{HeaderName, StatusCode},
    routing::get,
};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::{health, state::AppState};

static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

pub fn build_router(state: AppState) -> Router {
    let middleware = ServiceBuilder::new()
        .layer(SetRequestIdLayer::new(X_REQUEST_ID.clone(), MakeRequestUuid))
        .layer(TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
            let id = req.headers().get("x-request-id").and_then(|v| v.to_str().ok()).unwrap_or("-");
            tracing::info_span!("http", method = %req.method(), uri = %req.uri(), request_id = %id)
        }))
        .layer(PropagateRequestIdLayer::new(X_REQUEST_ID.clone()))
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(10)))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive());

    Router::new()
        .route("/health/live", get(health::liveness))
        .route("/health/ready", get(health::readiness))
        .layer(middleware)
        .with_state(state)
}
