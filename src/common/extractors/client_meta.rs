use std::{convert::Infallible, net::SocketAddr};

use axum::{
    extract::{ConnectInfo, FromRef, FromRequestParts},
    http::{header::USER_AGENT, request::Parts},
};

use crate::common::net::{self, TrustProxy};

/// Where a request came from, for showing to users ("sign-in requested from ...").
/// The IP honors `TRUST_PROXY_HEADERS` the same way the rate limiter does — see [`net::client_ip`].
#[derive(Debug, Clone, Default)]
pub struct ClientMeta {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

impl<S> FromRequestParts<S> for ClientMeta
where
    S: Send + Sync,
    TrustProxy: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TrustProxy(trust_proxy) = TrustProxy::from_ref(state);
        let ip = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| net::client_ip(&parts.headers, info.0, trust_proxy).to_string());
        let user_agent = parts
            .headers
            .get(USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|ua| ua.chars().take(200).collect());
        Ok(Self { ip, user_agent })
    }
}
