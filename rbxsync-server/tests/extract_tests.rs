//! Integration tests for the extraction pipeline's script-write boundary
//!
//! Drives /extract/chunk -> /extract/finalize (chunk auto-creates the
//! session) and /extract/start directly, asserting adopt-once semantics:
//! script sources are written only when no script file exists on disk.

use axum_test::TestServer;
use rbxsync_server::{create_router, AppState};
use serde_json::json;
use tempfile::TempDir;

fn create_test_server() -> TestServer {
    let state = AppState::new();
    let router = create_router(state);
    TestServer::new(router).unwrap()
}

fn write(root: &std::path::Path, rel: &str, content: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

fn script_instance(path: &str, class: &str, source: &str) -> serde_json::Value {
    json!({
        "className": class,
        "name": path.rsplit('/').next().unwrap(),
        "referenceId": format!("RBX{}", path.replace('/', "_")),
        "parentId": serde_json::Value::Null,
        "path": path,
        "properties": {"Source": {"type": "string", "value": source}}
    })
}

fn plain_instance(path: &str, class: &str) -> serde_json::Value {
    json!({
        "className": class,
        "name": path.rsplit('/').next().unwrap(),
        "referenceId": format!("RBX{}", path.replace('/', "_")),
        "parentId": serde_json::Value::Null,
        "path": path,
        "properties": {}
    })
}

async fn run_extraction(server: &TestServer, dir: &TempDir, instances: serde_json::Value) -> serde_json::Value {
    let project_dir = dir.path().to_string_lossy().to_string();
    let chunk = server
        .post("/extract/chunk")
        .json(&json!({
            "session_id": "extract-test",
            "chunk_index": 0,
            "total_chunks": 1,
            "data": instances,
            "project_dir": project_dir
        }))
        .await;
    chunk.assert_status_ok();
    let finalize = server.post("/extract/finalize").json(&json!({"project_dir": project_dir})).await;
    finalize.assert_status_ok();
    finalize.json()
}

fn adopted_of(body: &serde_json::Value) -> Vec<String> {
    body["adopted"]
        .as_array()
        .unwrap_or_else(|| panic!("finalize response missing adopted: {body:#?}"))
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn test_bootstrap_writes_scripts_and_sidecars() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    let body = run_extraction(&server, &dir, json!([
        script_instance("ServerScriptService/Main", "Script", "print('main')"),
        plain_instance("Workspace/Part", "Part"),
    ])).await;

    assert_eq!(body["success"], true);
    let main = dir.path().join("src/ServerScriptService/Main.server.luau");
    assert_eq!(std::fs::read_to_string(&main).unwrap(), "print('main')");
    let sidecar = std::fs::read_to_string(dir.path().join("src/ServerScriptService/Main.rbxjson")).unwrap();
    assert!(!sidecar.contains("\"Source\""));
    assert!(dir.path().join("src/Workspace/Part.rbxjson").exists());
    assert_eq!(adopted_of(&body), vec!["ServerScriptService/Main.server.luau"]);
    assert_eq!(body["scriptsWritten"], 1);
}

#[tokio::test]
async fn test_reextract_preserves_modified_script() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    let instances = json!([script_instance("ServerScriptService/Main", "Script", "print('studio')")]);
    run_extraction(&server, &dir, instances.clone()).await;

    let main = dir.path().join("src/ServerScriptService/Main.server.luau");
    std::fs::write(&main, "-- local edit").unwrap();

    let body = run_extraction(&server, &dir, instances).await;
    assert_eq!(std::fs::read_to_string(&main).unwrap(), "-- local edit");
    assert!(adopted_of(&body).is_empty());
    assert_eq!(body["scriptsWritten"], 0);
    // Sidecar still refreshed
    assert!(dir.path().join("src/ServerScriptService/Main.rbxjson").exists());
}

#[tokio::test]
async fn test_new_studio_script_adopted_once() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    let first = json!([script_instance("ServerScriptService/Main", "Script", "print('main')")]);
    run_extraction(&server, &dir, first).await;

    let both = json!([
        script_instance("ServerScriptService/Main", "Script", "print('main v2')"),
        script_instance("ReplicatedStorage/Fresh", "ModuleScript", "return {}"),
    ]);
    let body = run_extraction(&server, &dir, both.clone()).await;

    assert_eq!(adopted_of(&body), vec!["ReplicatedStorage/Fresh.luau"]);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/ServerScriptService/Main.server.luau")).unwrap(),
        "print('main')",
        "existing script must not be overwritten"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/ReplicatedStorage/Fresh.luau")).unwrap(),
        "return {}"
    );

    let body = run_extraction(&server, &dir, both).await;
    assert!(adopted_of(&body).is_empty(), "second extraction adopts nothing");
}

#[tokio::test]
async fn test_suffix_variant_blocks_adoption() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("src"), "ServerScriptService/Main.lua", "-- lua style");

    let body = run_extraction(&server, &dir, json!([
        script_instance("ServerScriptService/Main", "Script", "print('studio')"),
    ])).await;

    assert!(adopted_of(&body).is_empty());
    assert!(!dir.path().join("src/ServerScriptService/Main.server.luau").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/ServerScriptService/Main.lua")).unwrap(),
        "-- lua style"
    );
}

#[tokio::test]
async fn test_orphan_rbxjson_cleaned_scripts_survive() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    write(&src, "Workspace/Old.rbxjson", r#"{"className":"Part","name":"Old"}"#);
    write(&src, "ReplicatedStorage/Keep.luau", "return {}");

    run_extraction(&server, &dir, json!([plain_instance("Workspace/Part", "Part")])).await;

    assert!(!src.join("Workspace/Old.rbxjson").exists(), "orphaned instance file must be cleared");
    assert!(src.join("ReplicatedStorage/Keep.luau").exists(), "orphaned script must survive");
    assert!(src.join("Workspace/Part.rbxjson").exists());
}

#[tokio::test]
async fn test_extract_start_preserves_scripts() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    write(&src, "ServerScriptService/Main.server.luau", "print('keep me')");
    write(&src, "Workspace/Stale.rbxjson", r#"{"className":"Part","name":"Stale"}"#);

    let response = server
        .post("/extract/start")
        .json(&json!({"project_dir": dir.path().to_string_lossy()}))
        .await;
    response.assert_status_ok();

    assert_eq!(
        std::fs::read_to_string(src.join("ServerScriptService/Main.server.luau")).unwrap(),
        "print('keep me')"
    );
    assert!(!src.join("Workspace/Stale.rbxjson").exists());
    let backup = dir.path().join(".rbxsync-backup/src");
    assert!(backup.join("ServerScriptService/Main.server.luau").exists(), "backup keeps the original tree");
    assert!(backup.join("Workspace/Stale.rbxjson").exists());
}
