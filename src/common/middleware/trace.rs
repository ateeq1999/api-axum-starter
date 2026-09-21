use axum::http::Request;
use tracing::Span;

use super::request_id::X_REQUEST_ID;

pub fn make_span<B>(req: &Request<B>) -> Span {
    let id = req
        .headers()
        .get(&X_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    tracing::info_span!("http", method = %req.method(), uri = %req.uri(), request_id = %id)
}
