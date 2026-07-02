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
