use std::{convert::Infallible, net::SocketAddr};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{header::USER_AGENT, request::Parts},
};

/// Where a request came from, for showing to users ("sign-in requested from ...").
/// The IP is the TCP peer address, so behind a reverse proxy it is the proxy's address.
#[derive(Debug, Clone, Default)]
pub struct ClientMeta {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

impl<S: Send + Sync> FromRequestParts<S> for ClientMeta {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let ip = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip().to_string());
        let user_agent = parts
            .headers
            .get(USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|ua| ua.chars().take(200).collect());
        Ok(Self { ip, user_agent })
    }
}
