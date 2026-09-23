//! Resolving the real client address, optionally behind a single trusted reverse proxy.

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

/// Copy handle for whether this deployment sits behind a trusted reverse proxy. Lives in
/// `AppState` (see `TRUST_PROXY_HEADERS`), so extractors that need it can pull it via `FromRef`
/// without `common` depending on `config`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TrustProxy(pub bool);

/// The TCP peer's address, unless `trust_proxy` is set, in which case the right-most address in
/// `X-Forwarded-For` is used instead — the entry *this deployment's own* reverse proxy appended.
///
/// The right-most entry is the only one that can be trusted: a client can send any value it
/// likes as `X-Forwarded-For`, which a proxy then *appends* to, so every entry except the one the
/// trusted proxy itself added is attacker-controlled. This assumes exactly one trusted proxy hop
/// in front of this server (the common case for a starter behind a single load balancer/reverse
/// proxy); a chain of several trusted proxies would need a configurable hop count, which this
/// deliberately does not add.
pub fn client_ip(headers: &HeaderMap, peer: SocketAddr, trust_proxy: bool) -> IpAddr {
    if trust_proxy
        && let Some(forwarded) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok())
        && let Some(last) = forwarded.rsplit(',').next()
        && let Ok(ip) = last.trim().parse()
    {
        return ip;
    }
    peer.ip()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(forwarded_for: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", forwarded_for.parse().unwrap());
        headers
    }

    fn peer() -> SocketAddr {
        "10.0.0.1:12345".parse().unwrap()
    }

    #[test]
    fn uses_the_tcp_peer_when_proxy_is_not_trusted() {
        assert_eq!(
            client_ip(&headers("1.2.3.4"), peer(), false),
            "10.0.0.1".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn uses_the_rightmost_forwarded_address_when_trusted() {
        // A client-controlled entry, then the one this deployment's own proxy appended.
        assert_eq!(
            client_ip(&headers("203.0.113.9, 198.51.100.2"), peer(), true),
            "198.51.100.2".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn falls_back_to_the_tcp_peer_when_the_header_is_missing_or_unparsable() {
        assert_eq!(
            client_ip(&HeaderMap::new(), peer(), true),
            "10.0.0.1".parse::<IpAddr>().unwrap()
        );
        assert_eq!(
            client_ip(&headers("not-an-ip"), peer(), true),
            "10.0.0.1".parse::<IpAddr>().unwrap()
        );
    }
}
