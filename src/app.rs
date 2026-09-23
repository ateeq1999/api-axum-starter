use axum::Router;

use crate::{common::middleware::stack, infra, modules, state::AppState};

pub fn build_router(state: AppState) -> Router {
    // Applied before `stack::apply` so it wraps the routed handlers directly (closest to axum's
    // route matching, so `MatchedPath` labels are correct) and sits inside, not counted by, the
    // outer CORS/compression/timeout stack.
    let (metric_layer, _handle) = infra::metrics::layer_and_handle();
    let router = Router::new()
        .merge(modules::health::router())
        .nest("/api/v1", modules::router(&state))
        .layer(metric_layer);

    stack::apply(router).with_state(state)
}
