use axum::Json;
use serde_json::{Value, json};

pub async fn liveness() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
