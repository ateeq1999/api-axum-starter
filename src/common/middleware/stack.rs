use std::time::Duration;

use axum::{Router, http::StatusCode};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer,
};

use super::{request_id, trace};

/// Wraps a router with the global layers (request id, tracing, timeout, compression, CORS).
pub fn apply<S>(router: Router<S>) -> Router<S>
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
            .layer(CorsLayer::permissive()),
    )
}
