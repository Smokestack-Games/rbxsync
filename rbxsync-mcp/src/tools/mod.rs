use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// HTTP client for communicating with rbxsync-server
#[derive(Debug, Clone)]
pub struct RbxSyncClient {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractStartResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct ExtractStatusResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "chunksReceived")]
    pub chunks_received: i32,
    #[serde(rename = "totalChunks")]
    pub total_chunks: Option<i32>,
    pub complete: bool,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractFinalizeResponse {
    pub success: bool,
    #[serde(rename = "filesWritten")]
    pub files_written: i32,
    #[serde(rename = "scriptsWritten")]
    pub scripts_written: Option<i32>,
    #[serde(rename = "totalInstances")]
    pub total_instances: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SyncReadTreeResponse {
    pub instances: Vec<serde_json::Value>,  // Raw JSON instances
    pub count: i32,
}

#[derive(Debug, Deserialize)]
pub struct IncrementalSyncResponse {
    pub success: bool,
    pub instances: Vec<serde_json::Value>,
    pub count: i32,
    #[serde(default)]
    pub full_sync: bool,
    #[serde(default)]
    pub files_checked: usize,
    #[serde(default)]
    pub files_modified: usize,
    #[serde(default)]
    pub marked_synced: bool,
}

/// Build sync operations from raw instance data
/// Returns operations in the format expected by the plugin:
/// { type: "update", path: "...", data: { className, name, referenceId, attributes, properties, ... } }
pub fn build_sync_operations(instances: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    instances
        .into_iter()
        .filter_map(|inst| {
            let path = inst.get("path")?.as_str()?;
            Some(serde_json::json!({
                "type": "update",
                "path": path,
                "data": inst
            }))
        })
        .collect()
}

#[derive(Debug, Deserialize)]
pub struct SyncBatchResult {
    pub success: bool,
    #[serde(default)]
    pub skipped: bool,
}

#[derive(Debug, Deserialize)]
pub struct SyncBatchResponseData {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub applied: i32,
    #[serde(default)]
    pub skipped: i32,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub results: Vec<SyncBatchResult>,
    // Skip reason when sync is disabled or extraction in progress
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SyncBatchResponse {
    pub success: bool,
    #[serde(default)]
    pub data: Option<SyncBatchResponseData>,
    #[serde(default)]
    pub id: Option<String>,
    // Flattened fields for backwards compatibility
    #[serde(default)]
    pub applied: i32,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitStatusResponse {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub staged: Vec<String>,
    pub modified: Vec<String>,
    pub untracked: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitCommitResponse {
    pub success: bool,
    pub hash: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RunCodeResponse {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}

// Test runner types
#[derive(Debug, Serialize)]
pub struct TestRunParams {
    pub duration: Option<u32>,
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TestStartResponse {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConsoleMessage {
    pub message: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: f64,
}

#[derive(Debug, Deserialize)]
pub struct TestStatusResponse {
    #[serde(rename = "inProgress")]
    pub in_progress: bool,
    pub complete: bool,
    pub error: Option<String>,
    pub output: Vec<ConsoleMessage>,
    #[serde(rename = "totalMessages")]
    pub total_messages: i32,
}

#[derive(Debug, Deserialize)]
pub struct TestFinishResponse {
    pub success: bool,
    pub duration: Option<f64>,
    pub output: Vec<ConsoleMessage>,
    #[serde(rename = "totalMessages")]
    pub total_messages: i32,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommandResponse<T> {
    pub success: bool,
    pub data: T,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InsertModelResponse {
    pub success: bool,
    pub model_name: Option<String>,
    pub asset_id: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiffEntry {
    pub path: String,
    pub class_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiffResponse {
    pub added: Vec<DiffEntry>,
    pub removed: Vec<DiffEntry>,
    pub unchanged: usize,
}

impl RbxSyncClient {
    pub fn new(port: u16) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: format!("http://127.0.0.1:{}", port),
        }
    }

    pub async fn check_health(&self) -> anyhow::Result<bool> {
        let resp = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await?
            .json::<HealthResponse>()
            .await?;

        Ok(resp.status == "ok")
    }

    pub async fn start_extraction(
        &self,
        project_dir: &str,
        services: Option<&[String]>,
        include_terrain: bool,
    ) -> anyhow::Result<ExtractStartResponse> {
        let mut body = serde_json::json!({
            "project_dir": project_dir,
            "include_terrain": include_terrain
        });

        if let Some(services) = services {
            body["services"] = serde_json::json!(services);
        }

        let resp = self
            .client
            .post(format!("{}/extract/start", self.base_url))
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    pub async fn get_extraction_status(&self) -> anyhow::Result<ExtractStatusResponse> {
        let resp = self
            .client
            .get(format!("{}/extract/status", self.base_url))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    pub async fn finalize_extraction(
        &self,
        session_id: &str,
        project_dir: &str,
    ) -> anyhow::Result<ExtractFinalizeResponse> {
        let resp = self
            .client
            .post(format!("{}/extract/finalize", self.base_url))
            .json(&serde_json::json!({
                "session_id": session_id,
                "project_dir": project_dir
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    pub async fn read_tree(&self, project_dir: &str) -> anyhow::Result<SyncReadTreeResponse> {
        let resp = self
            .client
            .post(format!("{}/sync/read-tree", self.base_url))
            .json(&serde_json::json!({
                "project_dir": project_dir
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Read only files changed since last sync (incremental sync)
    pub async fn read_incremental(&self, project_dir: &str) -> anyhow::Result<IncrementalSyncResponse> {
        let resp = self
            .client
            .post(format!("{}/sync/incremental", self.base_url))
            .json(&serde_json::json!({
                "project_dir": project_dir
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Mark the project as synced (call after successful sync)
    pub async fn mark_synced(&self, project_dir: &str) -> anyhow::Result<()> {
        self.client
            .post(format!("{}/sync/incremental", self.base_url))
            .json(&serde_json::json!({
                "project_dir": project_dir,
                "mark_synced": true
            }))
            .send()
            .await?;

        Ok(())
    }

    pub async fn sync_batch(&self, operations: &[serde_json::Value]) -> anyhow::Result<SyncBatchResponse> {
        let resp = self
            .client
            .post(format!("{}/sync/batch", self.base_url))
            .json(&serde_json::json!({
                "operations": operations
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    pub async fn get_git_status(&self, project_dir: &str) -> anyhow::Result<GitStatusResponse> {
        let resp = self
            .client
            .post(format!("{}/git/status", self.base_url))
            .json(&serde_json::json!({
                "project_dir": project_dir
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    pub async fn git_commit(
        &self,
        project_dir: &str,
        message: &str,
        files: Option<&[String]>,
    ) -> anyhow::Result<GitCommitResponse> {
        let mut body = serde_json::json!({
            "project_dir": project_dir,
            "message": message
        });

        if let Some(files) = files {
            body["files"] = serde_json::json!(files);
        }

        let resp = self
            .client
            .post(format!("{}/git/commit", self.base_url))
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    pub async fn run_code(&self, code: &str) -> anyhow::Result<String> {
        let resp: RunCodeResponse = self
            .client
            .post(format!("{}/run", self.base_url))
            .json(&serde_json::json!({
                "code": code
            }))
            .send()
            .await?
            .json()
            .await?;

        if resp.success {
            Ok(resp.output.unwrap_or_default())
        } else {
            Ok(format!("Error: {}", resp.error.unwrap_or_default()))
        }
    }

    // Test runner methods
    pub async fn start_test(&self, duration: Option<u32>, mode: Option<&str>) -> anyhow::Result<TestStartResponse> {
        let mut payload = serde_json::json!({});
        if let Some(d) = duration {
            payload["duration"] = serde_json::json!(d);
        }
        if let Some(m) = mode {
            payload["mode"] = serde_json::json!(m);
        }

        let resp: CommandResponse<TestStartResponse> = self
            .client
            .post(format!("{}/sync/command", self.base_url))
            .json(&serde_json::json!({
                "command": "test:run",
                "payload": payload
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data)
    }

    pub async fn get_test_status(&self) -> anyhow::Result<TestStatusResponse> {
        let resp: CommandResponse<TestStatusResponse> = self
            .client
            .post(format!("{}/sync/command", self.base_url))
            .json(&serde_json::json!({
                "command": "test:status",
                "payload": {}
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data)
    }

    pub async fn finish_test(&self) -> anyhow::Result<TestFinishResponse> {
        let resp: CommandResponse<TestFinishResponse> = self
            .client
            .post(format!("{}/sync/command", self.base_url))
            .json(&serde_json::json!({
                "command": "test:finish",
                "payload": {}
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data)
    }

    pub async fn get_diff(&self, project_dir: &str) -> anyhow::Result<DiffResponse> {
        let resp = self
            .client
            .post(format!("{}/diff", self.base_url))
            .json(&serde_json::json!({
                "project_dir": project_dir
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    // ========================================================================
    // Bot Controller Methods (AI-powered automated gameplay testing)
    // ========================================================================

    /// Observe game state during playtest
    pub async fn bot_observe(
        &self,
        observe_type: &str,
        radius: Option<f64>,
        query: Option<&str>,
    ) -> anyhow::Result<BotCommandResponse> {
        let resp = self
            .client
            .post(format!("{}/bot/observe", self.base_url))
            .json(&serde_json::json!({
                "type": observe_type,
                "radius": radius,
                "query": query
            }))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Move character to position or object
    pub async fn bot_move(
        &self,
        position: Option<serde_json::Value>,
        object_name: Option<&str>,
    ) -> anyhow::Result<BotCommandResponse> {
        let resp = self
            .client
            .post(format!("{}/bot/move", self.base_url))
            .json(&serde_json::json!({
                "position": position,
                "objectName": object_name
            }))
            .timeout(std::time::Duration::from_secs(60)) // Longer timeout for movement
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Perform character action
    pub async fn bot_action(
        &self,
        action: &str,
        name: Option<&str>,
    ) -> anyhow::Result<BotCommandResponse> {
        let resp = self
            .client
            .post(format!("{}/bot/action", self.base_url))
            .json(&serde_json::json!({
                "action": action,
                "name": name
            }))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Send generic bot command
    pub async fn bot_command(
        &self,
        command_type: &str,
        command: &str,
        args: Option<serde_json::Value>,
    ) -> anyhow::Result<BotCommandResponse> {
        let resp = self
            .client
            .post(format!("{}/bot/command", self.base_url))
            .json(&serde_json::json!({
                "type": command_type,
                "command": command,
                "args": args.unwrap_or(serde_json::json!({}))
            }))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    // ========================================================================
    // Harness Methods (Multi-session AI game development tracking)
    // ========================================================================

    /// Initialize harness for a project
    pub async fn harness_init(
        &self,
        project_dir: &str,
        game_name: &str,
        description: Option<&str>,
        genre: Option<&str>,
    ) -> anyhow::Result<HarnessInitResponse> {
        let resp = self
            .client
            .post(format!("{}/harness/init", self.base_url))
            .json(&serde_json::json!({
                "projectDir": project_dir,
                "gameName": game_name,
                "description": description,
                "genre": genre
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Start a new development session
    pub async fn harness_session_start(
        &self,
        project_dir: &str,
        initial_goals: Option<&str>,
    ) -> anyhow::Result<SessionStartResponse> {
        let resp = self
            .client
            .post(format!("{}/harness/session/start", self.base_url))
            .json(&serde_json::json!({
                "projectDir": project_dir,
                "initialGoals": initial_goals
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// End a development session
    pub async fn harness_session_end(
        &self,
        project_dir: &str,
        session_id: &str,
        summary: Option<&str>,
        handoff_notes: Option<&[String]>,
    ) -> anyhow::Result<SessionEndResponse> {
        let resp = self
            .client
            .post(format!("{}/harness/session/end", self.base_url))
            .json(&serde_json::json!({
                "projectDir": project_dir,
                "sessionId": session_id,
                "summary": summary,
                "handoffNotes": handoff_notes
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Update or create a feature
    pub async fn harness_feature_update(
        &self,
        project_dir: &str,
        feature_id: Option<&str>,
        name: Option<&str>,
        description: Option<&str>,
        status: Option<&str>,
        priority: Option<&str>,
        tags: Option<&[String]>,
        add_note: Option<&str>,
        session_id: Option<&str>,
    ) -> anyhow::Result<FeatureUpdateResponse> {
        let resp = self
            .client
            .post(format!("{}/harness/feature/update", self.base_url))
            .json(&serde_json::json!({
                "projectDir": project_dir,
                "featureId": feature_id,
                "name": name,
                "description": description,
                "status": status,
                "priority": priority,
                "tags": tags,
                "addNote": add_note,
                "sessionId": session_id
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Get harness status for a project
    pub async fn harness_status(
        &self,
        project_dir: &str,
    ) -> anyhow::Result<HarnessStatusResponse> {
        let resp = self
            .client
            .post(format!("{}/harness/status", self.base_url))
            .json(&serde_json::json!({
                "projectDir": project_dir
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Read properties of an instance at the given path
    pub async fn read_properties(&self, path: &str) -> anyhow::Result<ReadPropertiesResponse> {
        let resp = self
            .client
            .post(format!("{}/read-properties", self.base_url))
            .json(&serde_json::json!({
                "path": path
            }))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Explore the game hierarchy
    pub async fn explore_hierarchy(
        &self,
        path: Option<&str>,
        depth: Option<u32>,
    ) -> anyhow::Result<ExploreHierarchyResponse> {
        let resp = self
            .client
            .post(format!("{}/explore-hierarchy", self.base_url))
            .json(&serde_json::json!({
                "path": path,
                "depth": depth.unwrap_or(1).min(10)
            }))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    /// Find instances matching search criteria
    pub async fn find_instances(
        &self,
        class_name: Option<&str>,
        name: Option<&str>,
        parent: Option<&str>,
        limit: Option<u32>,
    ) -> anyhow::Result<FindInstancesResponse> {
        let resp = self
            .client
            .post(format!("{}/find-instances", self.base_url))
            .json(&serde_json::json!({
                "className": class_name,
                "name": name,
                "parent": parent,
                "limit": limit.unwrap_or(100).min(1000)
            }))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }
}

// ============================================================================
// Bot Controller Response Types
// ============================================================================

/// Generic bot command response
#[derive(Debug, Deserialize)]
pub struct BotCommandResponse {
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

// ============================================================================
// Harness Response Types
// ============================================================================

/// Response from harness init
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessInitResponse {
    pub success: bool,
    pub message: String,
    pub harness_dir: String,
    pub game_id: Option<String>,
}

/// Response from session start
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
    pub session_path: Option<String>,
}

/// Response from session end
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEndResponse {
    pub success: bool,
    pub message: String,
}

/// Response from feature update
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureUpdateResponse {
    pub success: bool,
    pub message: String,
    pub feature_id: Option<String>,
}

/// Summary of feature statuses
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSummary {
    pub total: usize,
    pub planned: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub blocked: usize,
    pub cancelled: usize,
}

/// Brief session summary
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub summary: String,
    pub features_count: usize,
}

/// Response with harness status
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessStatusResponse {
    pub success: bool,
    pub initialized: bool,
    pub game: Option<serde_json::Value>,
    pub features: Vec<serde_json::Value>,
    pub feature_summary: FeatureSummary,
    pub recent_sessions: Vec<SessionSummary>,
}

/// Response from read_properties
#[derive(Debug, Deserialize)]
pub struct ReadPropertiesResponse {
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

/// Response from explore_hierarchy
#[derive(Debug, Deserialize)]
pub struct ExploreHierarchyResponse {
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

/// Response from find_instances
#[derive(Debug, Deserialize)]
pub struct FindInstancesResponse {
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}
