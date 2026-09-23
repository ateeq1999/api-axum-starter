use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::Response,
};

use crate::common::error::AppError;

const MAX_TRACKED_CLIENTS: usize = 10_000;

/// Fixed-window, in-memory, per-client-IP limiter for abusable public endpoints.
///
/// The key is the TCP peer address. Behind a reverse proxy every request shares the proxy's
/// address, so terminate rate limiting at the proxy or extend this to trust a forwarded header.
/// State is per process, so limits are not shared across multiple API replicas.
#[derive(Clone)]
pub struct RateLimiter {
    /// Distinguishes limiters in the `rate_limit_rejections_total` metric label.
    name: &'static str,
    max_requests: u32,
    window: Duration,
    hits: Arc<Mutex<HashMap<IpAddr, (Instant, u32)>>>,
}

impl RateLimiter {
    pub fn new(name: &'static str, max_requests: u32, window: Duration) -> Self {
        Self {
            name,
            max_requests,
            window,
            hits: Arc::default(),
        }
    }

    fn allow(&self, client: IpAddr, now: Instant) -> bool {
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        if hits.len() >= MAX_TRACKED_CLIENTS {
            hits.retain(|_, (started, _)| now.duration_since(*started) < self.window);
        }
        let entry = hits.entry(client).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= self.max_requests
    }

    /// How many distinct clients are currently tracked in this window. For the
    /// `rate_limiter_tracked_clients` gauge.
    pub fn tracked_clients(&self) -> usize {
        self.hits.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

pub async fn enforce(
    State(limiter): State<RateLimiter>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let client = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));

    if limiter.allow(client, Instant::now()) {
        Ok(next.run(req).await)
    } else {
        metrics::counter!("rate_limit_rejections_total", "limiter" => limiter.name).increment(1);
        Err(AppError::TooManyRequests)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_limit_and_resets_after_window() {
        let limiter = RateLimiter::new("test", 2, Duration::from_secs(60));
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        let t0 = Instant::now();
        assert!(limiter.allow(ip, t0));
        assert!(limiter.allow(ip, t0));
        assert!(!limiter.allow(ip, t0));
        assert!(limiter.allow("10.0.0.2".parse().unwrap(), t0));
        assert!(limiter.allow(ip, t0 + Duration::from_secs(61)));
    }
}
