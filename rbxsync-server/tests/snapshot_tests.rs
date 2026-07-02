//! Tests for the snapshot freshness endpoint

use axum_test::TestServer;
use rbxsync_server::{create_router, AppState};
use serde_json::json;

fn create_test_server() -> TestServer {
    let state = AppState::new();
    let router = create_router(state);
    TestServer::new(router).unwrap()
}

#[tokio::test]
async fn test_status_without_project_dir_is_bad_request() {
    let server = create_test_server();
    let response = server.get("/snapshot/status").await;
    response.assert_status_bad_request();
}

#[tokio::test]
async fn test_status_missing_file_reports_no_baseline() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    let response = server
        .get("/snapshot/status")
        .add_query_param("projectDir", dir.path().to_string_lossy())
        .await;
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["baseline"], false);
    assert_eq!(body["lastFullExtract"], serde_json::Value::Null);
    assert_eq!(body["lastLiveUpdate"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_status_reads_existing_freshness_file() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    let meta_dir = dir.path().join(".rbxsync");
    std::fs::create_dir_all(&meta_dir).unwrap();
    std::fs::write(
        meta_dir.join("snapshot.json"),
        json!({"lastFullExtract": 1700000000000u64, "lastLiveUpdate": 1700000001000u64}).to_string(),
    )
    .unwrap();

    let response = server
        .get("/snapshot/status")
        .add_query_param("projectDir", dir.path().to_string_lossy())
        .await;
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["baseline"], true);
    assert_eq!(body["lastFullExtract"], 1700000000000u64);
    assert_eq!(body["lastLiveUpdate"], 1700000001000u64);
    assert_eq!(body["placeId"], serde_json::Value::Null);
}
