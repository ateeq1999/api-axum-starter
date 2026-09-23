use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness))
}

#[utoipa::path(
    get,
    path = "/health/live",
    responses((status = 200, description = "The process is running")),
    tag = "health"
)]
pub(crate) async fn liveness() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

#[utoipa::path(
    get,
    path = "/health/ready",
    responses(
        (status = 200, description = "The database is reachable"),
        (status = 503, description = "The database is not reachable"),
    ),
    tag = "health"
)]
pub(crate) async fn readiness(State(db): State<PgPool>) -> (StatusCode, Json<Value>) {
    match sqlx::query("SELECT 1").execute(&db).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ready" }))),
        Err(e) => {
            tracing::warn!(error = ?e, "readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "database unavailable" })),
            )
        }
    }
}
