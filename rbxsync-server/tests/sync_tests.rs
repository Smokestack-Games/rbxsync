//! Integration tests for filesystem read/sync endpoints
//!
//! Covers /sync/read-tree, /sync/incremental, /sync/batch, and /diff
//! using the same TestServer pattern as harness_tests.rs.

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

/// Project fixture covering every path-mapping convention
fn fixture_project() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    write(&src, "ServerScriptService/Main.server.luau", "print('main')");
    write(&src, "ReplicatedStorage/Utils.luau", "return {}");
    write(&src, "StarterPlayer/Ctl.client.luau", "print('ctl')");
    write(&src, "Workspace/Part.rbxjson", r#"{"className":"Part","name":"Part"}"#);
    write(&src, "Workspace/Container/_meta.rbxjson", r#"{"className":"Model","name":"Container"}"#);
    write(&src, "ReplicatedStorage/Mod/init.luau", "return {}");
    dir
}

fn find_instance<'a>(instances: &'a [serde_json::Value], path: &str) -> &'a serde_json::Value {
    instances
        .iter()
        .find(|i| i["path"] == path)
        .unwrap_or_else(|| panic!("no instance with path {path}; got {instances:#?}"))
}

mod read_tree {
    use super::*;

    #[tokio::test]
    async fn test_read_tree_covers_all_conventions() {
        let server = create_test_server();
        let project = fixture_project();
        let response = server
            .post("/sync/read-tree")
            .json(&json!({"project_dir": project.path().to_string_lossy()}))
            .await;
        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["success"], true);
        let instances = body["instances"].as_array().unwrap();
        assert_eq!(body["count"], instances.len());

        let main = find_instance(instances, "ServerScriptService/Main");
        assert_eq!(main["className"], "Script");
        assert_eq!(main["properties"]["Source"]["value"], "print('main')");

        let ctl = find_instance(instances, "StarterPlayer/Ctl");
        assert_eq!(ctl["className"], "LocalScript");

        let utils = find_instance(instances, "ReplicatedStorage/Utils");
        assert_eq!(utils["className"], "ModuleScript");

        let part = find_instance(instances, "Workspace/Part");
        assert_eq!(part["className"], "Part");

        let container = find_instance(instances, "Workspace/Container");
        assert_eq!(container["className"], "Model");

        let module_dir = find_instance(instances, "ReplicatedStorage/Mod");
        assert_eq!(module_dir["className"], "ModuleScript");
    }

    #[tokio::test]
    async fn test_read_tree_missing_src_is_bad_request() {
        let server = create_test_server();
        let dir = tempfile::tempdir().unwrap();
        let response = server
            .post("/sync/read-tree")
            .json(&json!({"project_dir": dir.path().to_string_lossy()}))
            .await;
        response.assert_status_bad_request();
    }
}

mod incremental {
    use super::*;

    #[tokio::test]
    async fn test_incremental_full_then_filtered() {
        let server = create_test_server();
        let project = fixture_project();
        let project_dir = project.path().to_string_lossy().to_string();

        let first = server
            .post("/sync/incremental")
            .json(&json!({"project_dir": project_dir}))
            .await;
        first.assert_status_ok();
        let body: serde_json::Value = first.json();
        assert_eq!(body["full_sync"], true);
        assert!(body["count"].as_u64().unwrap() > 0);

        let marked = server
            .post("/sync/incremental")
            .json(&json!({"project_dir": project_dir, "mark_synced": true}))
            .await;
        marked.assert_status_ok();
        let body: serde_json::Value = marked.json();
        assert_eq!(body["marked_synced"], true);

        // Ensure the rewrite lands after the recorded sync time
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        write(
            &project.path().join("src"),
            "ReplicatedStorage/Utils.luau",
            "return {changed = true}",
        );

        let second = server
            .post("/sync/incremental")
            .json(&json!({"project_dir": project_dir}))
            .await;
        second.assert_status_ok();
        let body: serde_json::Value = second.json();
        assert_eq!(body["full_sync"], false);
        assert_eq!(body["files_modified"], 1);
        let instances = body["instances"].as_array().unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0]["path"], "ReplicatedStorage/Utils");
        assert_eq!(instances[0]["className"], "ModuleScript");
    }
}

mod batch {
    use super::*;

    #[tokio::test]
    async fn test_sync_batch_roundtrip_via_plugin_poll() {
        let server = create_test_server();
        let ops = vec![json!({
            "type": "update",
            "path": "ServerScriptService/Main",
            "data": {"className": "Script", "name": "Main", "source": "print('x')"}
        })];

        let batch = server
            .post("/sync/batch")
            .json(&json!({"operations": ops, "projectDir": "proj-a"}));

        let plugin = async {
            let poll = server
                .get("/rbxsync/request")
                .add_query_param("projectDir", "proj-a")
                .await;
            poll.assert_status_ok();
            let request: serde_json::Value = poll.json();
            assert_eq!(request["command"], "sync:batch");
            assert_eq!(request["payload"]["operations"].as_array().unwrap().len(), 1);
            let id = request["id"].as_str().unwrap().to_string();
            let ack = server
                .post("/rbxsync/response")
                .json(&json!({"id": id, "success": true, "data": {"applied": 1}}))
                .await;
            ack.assert_status_ok();
        };

        let (batch_response, _) = tokio::join!(batch, plugin);
        batch_response.assert_status_ok();
        let body: serde_json::Value = batch_response.json();
        assert_eq!(body["success"], true);
        assert_eq!(body["data"]["applied"], 1);
    }
}

mod tree_mapping {
    use super::*;

    #[tokio::test]
    async fn test_read_tree_applies_reverse_mapping() {
        let server = create_test_server();
        let project = fixture_project();
        std::fs::write(
            project.path().join("rbxsync.json"),
            r#"{"treeMapping": {"ReplicatedStorage/Shared": "shared"}}"#,
        )
        .unwrap();
        write(&project.path().join("src"), "shared/Util.luau", "return {}");

        let response = server
            .post("/sync/read-tree")
            .json(&json!({"project_dir": project.path().to_string_lossy()}))
            .await;
        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let instances = body["instances"].as_array().unwrap();
        let util = find_instance(instances, "ReplicatedStorage/Shared/Util");
        assert_eq!(util["className"], "ModuleScript");
        // Unmapped paths are untouched
        find_instance(instances, "ServerScriptService/Main");
    }

    #[tokio::test]
    async fn test_incremental_applies_reverse_mapping() {
        let server = create_test_server();
        let project = fixture_project();
        std::fs::write(
            project.path().join("rbxsync.json"),
            r#"{"treeMapping": {"ReplicatedStorage/Shared": "shared"}}"#,
        )
        .unwrap();
        write(&project.path().join("src"), "shared/Util.luau", "return {}");

        let response = server
            .post("/sync/incremental")
            .json(&json!({"project_dir": project.path().to_string_lossy()}))
            .await;
        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["full_sync"], true);
        let instances = body["instances"].as_array().unwrap();
        let util = find_instance(instances, "ReplicatedStorage/Shared/Util");
        assert_eq!(util["className"], "ModuleScript");
        assert!(!instances.iter().any(|i| i["path"] == "shared/Util"));
    }

    #[tokio::test]
    async fn test_diff_applies_reverse_mapping() {
        let server = create_test_server();
        let project = fixture_project();
        std::fs::write(
            project.path().join("rbxsync.json"),
            r#"{"treeMapping": {"ReplicatedStorage/Shared": "shared"}}"#,
        )
        .unwrap();
        write(
            &project.path().join("src"),
            "shared/Thing.rbxjson",
            r#"{"className":"Folder","name":"Thing"}"#,
        );

        let diff = server
            .post("/diff")
            .json(&json!({"project_dir": project.path().to_string_lossy()}));

        let plugin = async {
            let poll = server.get("/rbxsync/request").await;
            poll.assert_status_ok();
            let request: serde_json::Value = poll.json();
            assert_eq!(request["command"], "studio:paths");
            let id = request["id"].as_str().unwrap().to_string();
            server
                .post("/rbxsync/response")
                .json(&json!({
                    "id": id,
                    "success": true,
                    "data": {"paths": [
                        {"path": "Workspace/Part", "className": "Part"}
                    ]}
                }))
                .await
                .assert_status_ok();
        };

        let (diff_response, _) = tokio::join!(diff, plugin);
        diff_response.assert_status_ok();
        let body: serde_json::Value = diff_response.json();
        assert_eq!(body["success"], true);
        let added: Vec<&str> = body["added"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap()).collect();
        assert!(added.contains(&"ReplicatedStorage/Shared/Thing"));
        assert!(!added.contains(&"shared/Thing"));
    }
}

mod diff {
    use super::*;

    #[tokio::test]
    async fn test_diff_compares_rbxjson_files_with_studio_paths() {
        let server = create_test_server();
        let project = fixture_project();

        let diff = server
            .post("/diff")
            .json(&json!({"project_dir": project.path().to_string_lossy()}));

        let plugin = async {
            let poll = server.get("/rbxsync/request").await;
            poll.assert_status_ok();
            let request: serde_json::Value = poll.json();
            assert_eq!(request["command"], "studio:paths");
            let id = request["id"].as_str().unwrap().to_string();
            server
                .post("/rbxsync/response")
                .json(&json!({
                    "id": id,
                    "success": true,
                    "data": {"paths": [
                        {"path": "Workspace/Part", "className": "Part"},
                        {"path": "Workspace/OnlyInStudio", "className": "Folder"}
                    ]}
                }))
                .await
                .assert_status_ok();
        };

        let (diff_response, _) = tokio::join!(diff, plugin);
        diff_response.assert_status_ok();
        let body: serde_json::Value = diff_response.json();
        assert_eq!(body["success"], true);
        // File side counts only .rbxjson entries: Workspace/Part + Workspace/Container
        let added: Vec<&str> = body["added"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap()).collect();
        let removed: Vec<&str> = body["removed"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap()).collect();
        assert!(added.contains(&"Workspace/Container"));
        assert!(removed.contains(&"Workspace/OnlyInStudio"));
        assert_eq!(body["common"], 1);
    }
}

mod from_studio {
    use super::*;

    /// A minimal project with a `src` directory (the from-studio handler
    /// requires `src` to exist). datamodel.rbxjson starts absent and is
    /// materialized by the debounced flush.
    fn script_project() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        dir
    }

    /// Server plus the Arc<AppState> it was built from, so tests can drive
    /// `flush_project` deterministically instead of waiting on the sweeper.
    fn fixture_server() -> (TestServer, std::sync::Arc<rbxsync_server::AppState>, TempDir) {
        let state = AppState::new();
        let server = TestServer::new(create_router(state.clone())).unwrap();
        (server, state, script_project())
    }

    async fn post_ops(server: &TestServer, dir: &TempDir, ops: serde_json::Value) {
        server
            .post("/sync/from-studio")
            .json(&json!({"operations": ops, "projectDir": dir.path().to_string_lossy()}))
            .await
            .assert_status_ok();
    }

    fn read_datamodel(dir: &TempDir) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("datamodel.rbxjson")).unwrap()).unwrap()
    }

    fn child<'a>(node: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
        node.get("children")?.as_array()?.iter().find(|c| c["name"] == name)
    }

    #[tokio::test]
    async fn test_create_writes_node_without_source() {
        let (server, state, dir) = fixture_server();
        post_ops(&server, &dir, json!([{
            "type": "create", "path": "ServerScriptService/Fresh", "className": "Script",
            "data": {"className": "Script", "name": "Fresh", "path": "ServerScriptService/Fresh",
                "properties": {"Source": {"type": "string", "value": "print(1)"},
                               "Enabled": {"type": "bool", "value": true}}}
        }])).await;
        state.flush_project(&dir.path().to_string_lossy()).await;

        let doc = read_datamodel(&dir);
        let sss = child(&doc, "ServerScriptService").expect("ServerScriptService present");
        let fresh = child(sss, "Fresh").expect("Fresh present");
        assert!(fresh["properties"].get("Source").is_none(), "Source stripped from context node");
        assert_eq!(fresh["properties"]["Enabled"], json!({"type": "bool", "value": true}));
    }

    #[tokio::test]
    async fn test_modify_updates_node() {
        let (server, state, dir) = fixture_server();
        let project = dir.path().to_string_lossy().to_string();

        post_ops(&server, &dir, json!([{
            "type": "create", "path": "Workspace/Item", "className": "Folder",
            "data": {"className": "Folder", "name": "Item",
                "properties": {"Flag": {"type": "bool", "value": false}}}
        }])).await;
        state.flush_project(&project).await;
        let before = read_datamodel(&dir);
        assert_eq!(child(child(&before, "Workspace").unwrap(), "Item").unwrap()["properties"]["Flag"],
            json!({"type": "bool", "value": false}));

        post_ops(&server, &dir, json!([{
            "type": "modify", "path": "Workspace/Item", "className": "Folder",
            "data": {"className": "Folder", "name": "Item",
                "properties": {"Flag": {"type": "bool", "value": true}}}
        }])).await;
        state.flush_project(&project).await;

        let after = read_datamodel(&dir);
        assert_eq!(child(child(&after, "Workspace").unwrap(), "Item").unwrap()["properties"]["Flag"],
            json!({"type": "bool", "value": true}), "modify updates the node's properties");
    }

    #[tokio::test]
    async fn test_delete_removes_node() {
        let (server, state, dir) = fixture_server();
        let project = dir.path().to_string_lossy().to_string();

        post_ops(&server, &dir, json!([{
            "type": "create", "path": "Workspace/Gone", "className": "Part",
            "data": {"className": "Part", "name": "Gone", "properties": {}}
        }])).await;
        state.flush_project(&project).await;
        assert!(child(child(&read_datamodel(&dir), "Workspace").unwrap(), "Gone").is_some());

        post_ops(&server, &dir, json!([{
            "type": "delete", "path": "Workspace/Gone", "className": "Part", "data": {}
        }])).await;
        state.flush_project(&project).await;

        let doc = read_datamodel(&dir);
        assert!(child(child(&doc, "Workspace").unwrap(), "Gone").is_none(), "deleted node removed from context");
    }

    #[tokio::test]
    async fn test_rename_moves_node() {
        let (server, state, dir) = fixture_server();
        let project = dir.path().to_string_lossy().to_string();

        post_ops(&server, &dir, json!([{
            "type": "create", "path": "Workspace/Old", "className": "Model",
            "data": {"className": "Model", "name": "Old", "properties": {}}
        }])).await;
        state.flush_project(&project).await;

        post_ops(&server, &dir, json!([{
            "type": "rename", "path": "Workspace/New", "className": "Model",
            "data": {"oldPath": "Workspace/Old", "newPath": "Workspace/New"}
        }])).await;
        state.flush_project(&project).await;

        let ws = child(&read_datamodel(&dir), "Workspace").unwrap().clone();
        assert!(child(&ws, "New").is_some(), "renamed node present at new path");
        assert!(child(&ws, "Old").is_none(), "old path removed after rename");
    }

    #[tokio::test]
    async fn test_flush_stamps_freshness_only_when_ops_apply() {
        let (server, state, dir) = fixture_server();
        let project = dir.path().to_string_lossy().to_string();
        let meta = dir.path().join(".rbxsync/snapshot.json");

        // Delete of a nonexistent path applies nothing -> no freshness stamp.
        post_ops(&server, &dir, json!([{
            "type": "delete", "path": "Workspace/DoesNotExist", "className": "Part", "data": {}
        }])).await;
        state.flush_project(&project).await;
        assert!(!meta.exists(), "no-op batch must not stamp freshness");

        // A real write stamps lastLiveUpdate but never creates a baseline.
        post_ops(&server, &dir, json!([{
            "type": "modify", "path": "Workspace/Part", "className": "Part",
            "data": {"className": "Part", "name": "Part", "properties": {}}
        }])).await;
        state.flush_project(&project).await;
        let doc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&meta).unwrap()).unwrap();
        assert!(doc["lastLiveUpdate"].as_u64().unwrap() > 0);
        assert!(doc.get("lastFullExtract").and_then(|v| v.as_u64()).is_none());
    }

    #[tokio::test]
    async fn test_recently_synced_ttl_evicts() {
        let (_server, state, _dir) = fixture_server();
        let p = std::path::PathBuf::from("Workspace_ttl_probe.rbxjson");
        state.mark_recently_synced(p.clone()).await;
        assert!(state.is_recently_synced(&p).await);

        // An entry older than the TTL is evicted on the next check
        if let Some(old) = std::time::Instant::now().checked_sub(std::time::Duration::from_secs(3)) {
            state.recently_synced.write().await.insert(p.clone(), old);
            assert!(!state.is_recently_synced(&p).await);
        }
    }

    #[tokio::test]
    async fn test_flush_write_failure_rebuffers_ops_for_retry() {
        let (server, state, dir) = fixture_server();
        let project = dir.path().to_string_lossy().to_string();

        // Force the write side of the flush to fail deterministically: the atomic
        // write path is `datamodel.rbxjson.tmp`, so pre-creating a directory at
        // that exact path makes `std::fs::write` fail on both Windows and POSIX
        // (you cannot write file contents to a path that is itself a directory).
        let tmp_path = dir.path().join("datamodel.rbxjson.tmp");
        std::fs::create_dir(&tmp_path).unwrap();

        post_ops(&server, &dir, json!([{
            "type": "create", "path": "Workspace/Survivor", "className": "Part",
            "data": {"className": "Part", "name": "Survivor", "properties": {}}
        }])).await;
        state.flush_project(&project).await;

        // The write failed, so no datamodel.rbxjson should have been produced...
        assert!(!dir.path().join("datamodel.rbxjson").exists(), "failed write must not produce a partial file");
        // ...and the drained op must be re-buffered rather than dropped.
        {
            let map = state.pending_flush.read().await;
            let pending = map.get(&project).expect("ops re-buffered after failed write");
            assert_eq!(pending.ops.len(), 1, "the live delta must survive the failed flush");
        }

        // Clear the obstruction and retry: the re-buffered op is applied on the next flush.
        std::fs::remove_dir(&tmp_path).unwrap();
        state.flush_project(&project).await;

        let doc = read_datamodel(&dir);
        assert!(child(child(&doc, "Workspace").unwrap(), "Survivor").is_some(),
            "re-buffered op is applied once the write can succeed");
    }
}
