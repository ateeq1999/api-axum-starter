use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    common::middleware::stack,
    infra::{self, openapi::ApiDoc},
    modules,
    state::AppState,
};

pub fn build_router(state: AppState) -> Router {
    // Applied before `stack::apply` so it wraps the routed handlers directly (closest to axum's
    // route matching, so `MatchedPath` labels are correct) and sits inside, not counted by, the
    // outer CORS/compression/timeout stack.
    let (metric_layer, _handle) = infra::metrics::layer_and_handle();
    let router = Router::new()
        .merge(modules::health::router())
        .nest("/api/v1", modules::router(&state))
        .layer(metric_layer);

    let cors_allowed_origins = state.config.cors_allowed_origins.clone();
    let max_request_body_bytes = state.config.max_request_body_bytes;
    let app = stack::apply(router, &cors_allowed_origins, max_request_body_bytes).with_state(state);

    // Swagger UI at /docs, the raw spec at /api-docs/openapi.json. Merged after `with_state` (it
    // needs none of `AppState`) so it is unaffected by the API's CORS/compression/timeout stack.
    app.merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
}
