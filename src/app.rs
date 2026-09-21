use axum::Router;

use crate::{common::middleware::stack, modules, state::AppState};

pub fn build_router(state: AppState) -> Router {
    let router = Router::new()
        .merge(modules::health::router())
        .nest("/api/v1", modules::router(&state));

    stack::apply(router).with_state(state)
}
