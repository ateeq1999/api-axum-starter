//! The API contract, frozen: the OpenAPI document must not change unless a change is intended.
//! After an intended change, regenerate the snapshot with:
//!
//!   UPDATE_OPENAPI_SNAPSHOT=1 cargo test --test openapi_contract

use api_starter_axum::infra::openapi;

const SNAPSHOT: &str = "tests/snapshots/openapi.json";

#[test]
fn the_openapi_document_matches_the_committed_snapshot() {
    let current = serde_json::to_string_pretty(&openapi::document()).unwrap() + "\n";
    if std::env::var_os("UPDATE_OPENAPI_SNAPSHOT").is_some() {
        std::fs::write(SNAPSHOT, &current).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(SNAPSHOT)
        .expect("no snapshot yet: run with UPDATE_OPENAPI_SNAPSHOT=1 to create it")
        .replace("\r\n", "\n");
    assert!(
        current == committed,
        "the OpenAPI document changed; if intended, regenerate {SNAPSHOT} (see this file's header)"
    );
}
