//! Tests for project-dir key normalization in request routing
//!
//! Studio (backslashes), VS Code (forward slashes), and mixed drive-letter
//! case must all resolve to one queue/registry identity.

use axum_test::TestServer;
use rbxsync_server::{create_router, AppState};
use serde_json::json;

fn create_test_server() -> TestServer {
    let state = AppState::new();
    let router = create_router(state);
    TestServer::new(router).unwrap()
}

#[tokio::test]
async fn test_link_studio_normalizes_project_dir() {
    let server = create_test_server();

    let register = server
        .post("/rbxsync/register")
        .json(&json!({
            "place_id": 42u64,
            "place_name": "RoutingTest",
            "project_dir": "C:/Users/rt/proj",
            "session_id": "sess-routing-1"
        }))
        .await;
    register.assert_status_ok();

    // Link with a backslash, capital-drive variant of another dir
    let link = server
        .post("/rbxsync/link-studio")
        .json(&json!({
            "place_id": 42i64,
            "new_project_dir": "C:\\Users\\rt\\other"
        }))
        .await;
    link.assert_status_ok();

    let places = server.get("/rbxsync/places").await;
    places.assert_status_ok();
    let body: serde_json::Value = places.json();
    let dir = body["places"][0]["project_dir"].as_str().unwrap();
    assert_eq!(
        dir, "c:/Users/rt/other",
        "stored project_dir must be normalized (forward slashes, lowercase drive)"
    );
}

#[tokio::test]
async fn test_register_normalizes_drive_letter_case() {
    let server = create_test_server();

    let register = server
        .post("/rbxsync/register")
        .json(&json!({
            "place_id": 43u64,
            "place_name": "RoutingTest2",
            "project_dir": "C:\\Users\\rt\\proj2",
            "session_id": "sess-routing-2"
        }))
        .await;
    register.assert_status_ok();

    let places = server.get("/rbxsync/places").await;
    let body: serde_json::Value = places.json();
    let dir = body["places"][0]["project_dir"].as_str().unwrap();
    assert_eq!(dir, "c:/Users/rt/proj2");
}

#[tokio::test]
async fn test_operation_status_matches_across_path_styles() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    // Build a backslash, capital-drive representation of the temp dir
    let raw = dir.path().to_string_lossy().to_string();
    let backslashed = raw.replace('/', "\\");

    let start = server
        .post("/extract/start")
        .json(&json!({"project_dir": backslashed}))
        .await;
    start.assert_status_ok();

    // Query with the forward-slash, lowercase-drive form
    let mut forward = raw.replace('\\', "/");
    if forward.len() >= 2 && forward.as_bytes()[1] == b':' {
        let lower = forward.as_bytes()[0].to_ascii_lowercase() as char;
        forward.replace_range(0..1, &lower.to_string());
    }
    let status = server
        .get("/rbxsync/status")
        .add_query_param("projectDir", &forward)
        .await;
    status.assert_status_ok();
    let body: serde_json::Value = status.json();
    assert!(
        !body["operation"].is_null(),
        "operation keyed by one path style must be visible via the other: {body:#?}"
    );
}

#[tokio::test]
async fn test_health_reports_version() {
    let server = create_test_server();

    let health = server.get("/health").await;
    health.assert_status_ok();
    let body: serde_json::Value = health.json();
    assert_eq!(body["status"], "ok");
    assert!(
        body["version"].as_str().is_some_and(|v| !v.is_empty()),
        "health must report a non-empty version: {body:#?}"
    );
}

#[tokio::test]
async fn test_registered_workspace_is_listed() {
    let server = create_test_server();

    let register = server
        .post("/rbxsync/register-vscode")
        .json(&json!({ "workspace_dir": "C:/Users/rt/ws" }))
        .await;
    register.assert_status_ok();

    let workspaces = server.get("/rbxsync/workspaces").await;
    workspaces.assert_status_ok();
    let body: serde_json::Value = workspaces.json();
    let dirs = body["workspaces"].as_array().unwrap();
    assert!(
        dirs.iter().any(|d| d == "c:/Users/rt/ws"),
        "registered workspace must appear in the list: {body:#?}"
    );
}
