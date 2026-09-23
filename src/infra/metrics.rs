//! Prometheus metrics: automatic HTTP metrics via `axum-prometheus`, plus a handful of
//! business/domain counters and gauges recorded at the call sites that matter (see the
//! `metrics::counter!`/`gauge!` calls throughout `modules/`).
//!
//! `/metrics` is served on its own listener (`serve`), deliberately outside the main app's
//! router and middleware stack: it needs no CORS, no compression, no request timeout, and must
//! not count itself in `http_requests_total`.

use std::{sync::OnceLock, time::Duration};

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use axum_prometheus::PrometheusMetricLayer;
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;
use tokio::net::TcpListener;

use crate::{common::middleware::rate_limit::RateLimiter, common::security::password};

type Pair = (PrometheusMetricLayer<'static>, PrometheusHandle);

static METRICS: OnceLock<Pair> = OnceLock::new();

/// The HTTP metrics layer and its handle. Installs the global `metrics` recorder on first call
/// and reuses it on every later call — safe to call once per production process, and safe to
/// call again for every `TestApp` a test binary builds (installing the recorder twice would
/// otherwise panic).
pub fn layer_and_handle() -> Pair {
    METRICS.get_or_init(PrometheusMetricLayer::pair).clone()
}

struct MetricsState {
    handle: PrometheusHandle,
    token: Option<String>,
}

/// The standalone `/metrics` app. Bind and serve it separately from the main router.
fn router(handle: PrometheusHandle, token: Option<String>) -> Router {
    Router::new()
        .route("/metrics", get(render))
        .with_state(std::sync::Arc::new(MetricsState { handle, token }))
}

async fn render(State(state): State<std::sync::Arc<MetricsState>>, headers: HeaderMap) -> Response {
    if let Some(expected) = &state.token {
        let presented = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        if presented != Some(expected.as_str()) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    state.handle.render().into_response()
}

/// Binds and serves `/metrics` until the process shuts down. Runs as its own background task.
pub async fn serve(
    bind_addr: std::net::SocketAddr,
    handle: PrometheusHandle,
    token: Option<String>,
) {
    let listener = match TcpListener::bind(bind_addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(%error, %bind_addr, "could not bind the metrics listener");
            return;
        }
    };
    tracing::info!(%bind_addr, "metrics listening");
    if let Err(error) = axum::serve(listener, router(handle, token)).await {
        tracing::error!(%error, "metrics server stopped unexpectedly");
    }
}

/// Samples point-in-time gauges (DB pool usage, rate limiter size, hash-permit availability)
/// every 15s for as long as the process runs. Fire-and-forget: intentionally never joined.
pub fn spawn_gauge_sampler(db: PgPool, rate_limiter: RateLimiter) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(15));
        loop {
            tick.tick().await;
            metrics::gauge!("db_pool_connections").set(db.size() as f64);
            metrics::gauge!("db_pool_idle_connections").set(db.num_idle() as f64);
            metrics::gauge!("rate_limiter_tracked_clients")
                .set(rate_limiter.tracked_clients() as f64);
            metrics::gauge!("password_hash_permits_available")
                .set(password::available_permits() as f64);
        }
    });
}
