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
async fn test_bootstrap_writes_scripts_and_context_file() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    let body = run_extraction(&server, &dir, json!([
        script_instance("ServerScriptService/Main", "Script", "print('main')"),
        plain_instance("Workspace/Part", "Part"),
    ])).await;

    assert_eq!(body["success"], true);
    let main = dir.path().join("src/ServerScriptService/Main.server.luau");
    assert_eq!(std::fs::read_to_string(&main).unwrap(), "print('main')");
    // Non-script state lives in the single datamodel.rbxjson context document at
    // the project root, not in per-instance sidecars. Scripts carry a sourcePath,
    // never their Source.
    let context = std::fs::read_to_string(dir.path().join("datamodel.rbxjson")).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&context).unwrap();
    assert_eq!(doc["className"], "DataModel");
    assert!(context.contains("\"sourcePath\""));
    assert!(context.contains("ServerScriptService/Main.server.luau"));
    assert!(!context.contains("\"Source\""), "script Source must be stripped from the context doc");
    assert!(context.contains("\"Part\""), "non-script instances live in the context doc");
    assert!(!dir.path().join("src/ServerScriptService/Main.rbxjson").exists(), "no per-instance sidecars");
    assert!(!dir.path().join("src/Workspace/Part.rbxjson").exists(), "no per-instance sidecars");
    assert_eq!(adopted_of(&body), vec!["ServerScriptService/Main.server.luau"]);
    assert_eq!(body["scriptsWritten"], 1);

    let leftover_chunks: Vec<_> = std::fs::read_dir(dir.path().join("src"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("chunk_"))
        .collect();
    assert!(leftover_chunks.is_empty(), "chunk files must be cleaned after finalize: {leftover_chunks:?}");
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
    // Context document refreshed each extraction
    assert!(dir.path().join("datamodel.rbxjson").exists());
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
    assert!(dir.path().join("datamodel.rbxjson").exists(), "extracted tree written to the context doc");
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

#[tokio::test]
async fn test_start_then_finalize_prepares_once() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    write(&src, "ServerScriptService/Main.server.luau", "print('original')");
    write(&src, "Workspace/Stale.rbxjson", r#"{"className":"Part","name":"Stale"}"#);
    let project_dir = dir.path().to_string_lossy().to_string();

    let start = server.post("/extract/start").json(&json!({"project_dir": project_dir})).await;
    start.assert_status_ok();

    // A local edit between start and finalize must survive: finalize skips
    // its own prepare because start already ran it
    std::fs::write(src.join("ServerScriptService/Main.server.luau"), "-- edited after start").unwrap();

    let chunk = server.post("/extract/chunk").json(&json!({
        "session_id": "gate-test", "chunk_index": 0, "total_chunks": 1,
        "data": json!([script_instance("ServerScriptService/Main", "Script", "print('studio')")]),
        "project_dir": project_dir
    })).await;
    chunk.assert_status_ok();
    let finalize = server.post("/extract/finalize").json(&json!({"project_dir": project_dir})).await;
    finalize.assert_status_ok();

    assert_eq!(
        std::fs::read_to_string(src.join("ServerScriptService/Main.server.luau")).unwrap(),
        "-- edited after start"
    );
    // Backup reflects the pre-start tree, not a mid-extraction re-backup
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".rbxsync-backup/src/ServerScriptService/Main.server.luau")).unwrap(),
        "print('original')"
    );
    assert!(dir.path().join(".rbxsync-backup/src/Workspace/Stale.rbxjson").exists());
}

#[tokio::test]
async fn test_start_without_project_dir_does_not_mark_prepared() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    write(&src, "Workspace/Stale.rbxjson", r#"{"className":"Part","name":"Stale"}"#);
    let project_dir = dir.path().to_string_lossy().to_string();

    // start with no project_dir: session exists but nothing was prepared
    let start = server.post("/extract/start").json(&json!({})).await;
    start.assert_status_ok();

    let chunk = server.post("/extract/chunk").json(&json!({
        "session_id": "gate-test-2", "chunk_index": 0, "total_chunks": 1,
        "data": json!([plain_instance("Workspace/Part", "Part")]),
        "project_dir": project_dir
    })).await;
    chunk.assert_status_ok();
    let finalize = server.post("/extract/finalize").json(&json!({"project_dir": project_dir})).await;
    finalize.assert_status_ok();

    // Finalize must have run its own prepare: stale instance file cleared
    assert!(!src.join("Workspace/Stale.rbxjson").exists());
    assert!(dir.path().join("datamodel.rbxjson").exists());
}

#[tokio::test]
async fn test_finalize_writes_freshness_file() {
    let server = create_test_server();
    let dir = tempfile::tempdir().unwrap();
    run_extraction(&server, &dir, json!([plain_instance("Workspace/Part", "Part")])).await;

    let meta = dir.path().join(".rbxsync/snapshot.json");
    let doc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&meta).unwrap()).unwrap();
    assert!(doc["lastFullExtract"].as_u64().unwrap() > 0);
    assert!(doc["lastLiveUpdate"].as_u64().unwrap() > 0);
    assert!(!dir.path().join(".rbxsync/snapshot.json.tmp").exists(), "atomic write must not leave a temp file");
}
