use serde::{Deserialize, Serialize};

/// Check if debug mode is enabled via RBXSYNC_DEBUG env var
fn is_debug_enabled() -> bool {
    std::env::var("RBXSYNC_DEBUG").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false)
}

/// Log raw response body for debugging
fn debug_log_response(endpoint: &str, body: &str) {
    if is_debug_enabled() {
        eprintln!("[RBXSYNC_DEBUG] {} response: {}", endpoint, body);
    }
}

/// HTTP client for communicating with rbxsync-server
#[derive(Debug, Clone)]
pub struct RbxSyncClient {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
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

/// A changed file with its status (matches server's ChangedFile)
#[derive(Debug, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub status: String, // "modified", "added", "deleted", "renamed", "untracked", "staged"
}

/// Git status from server (matches server's GitStatus struct)
#[derive(Debug, Default, Deserialize)]
pub struct ServerGitStatus {
    pub branch: String,
    #[serde(default)]
    pub is_dirty: bool,
    #[serde(default)]
    pub staged_count: usize,
    #[serde(default)]
    pub unstaged_count: usize,
    #[serde(default)]
    pub untracked_count: usize,
    #[serde(default)]
    pub ahead: usize,
    #[serde(default)]
    pub behind: usize,
    #[serde(default)]
    pub changed_files: Vec<ChangedFile>,
}

/// Processed git status for MCP tool output
#[derive(Debug)]
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
    #[serde(default)]
    pub message: String,
    #[serde(rename = "type", default)]
    pub msg_type: String,
    #[serde(default)]
    pub timestamp: f64,
}

#[derive(Debug, Deserialize)]
pub struct TestStatusResponse {
    #[serde(rename = "inProgress", default)]
    pub in_progress: bool,
    #[serde(default)]
    pub complete: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub output: Vec<ConsoleMessage>,
    #[serde(rename = "totalMessages", default)]
    pub total_messages: i32,
}

#[derive(Debug, Deserialize)]
pub struct TestFinishResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub output: Vec<ConsoleMessage>,
    #[serde(rename = "totalMessages", default)]
    pub total_messages: i32,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TestStopResponse {
    pub success: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Generic command response from server
/// Note: Server returns PluginResponse which has id, success, data, error
/// The `data` field contains the actual response payload
#[derive(Debug, Deserialize)]
pub struct CommandResponse<T> {
    pub success: bool,
    #[serde(default)]
    pub data: Option<T>,
    #[serde(default)]
    pub error: Option<String>,
    /// UUID from server (ignored but must be accepted)
    #[serde(default)]
    pub id: Option<String>,
}

/// Raw plugin response for flexible parsing
#[derive(Debug, Deserialize)]
pub struct RawPluginResponse {
    pub success: bool,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertModelResponse {
    pub success: bool,
    pub inserted_name: Option<String>,
    pub inserted_path: Option<String>,
    pub class_name: Option<String>,
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
        let url = format!("{}/health", self.base_url);
        let response = self.client.get(&url).send().await?;
        let body = response.text().await?;
        debug_log_response("check_health", &body);

        let resp: HealthResponse = serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse health response: {}. Body: {}", e, body))?;

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

    pub async fn sync_batch(&self, operations: &[serde_json::Value], project_dir: Option<&str>) -> anyhow::Result<SyncBatchResponse> {
        let resp = self
            .client
            .post(format!("{}/sync/batch", self.base_url))
            .json(&serde_json::json!({
                "operations": operations,
                "projectDir": project_dir
            }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    pub async fn get_git_status(&self, project_dir: &str) -> anyhow::Result<GitStatusResponse> {
        let resp: CommandResponse<ServerGitStatus> = self
            .client
            .post(format!("{}/git/status", self.base_url))
            .json(&serde_json::json!({
                "project_dir": project_dir
            }))
            .send()
            .await?
            .json()
            .await?;

        if !resp.success {
            // Not a git repo or other error
            return Ok(GitStatusResponse {
                is_repo: false,
                branch: None,
                staged: vec![],
                modified: vec![],
                untracked: vec![],
            });
        }

        let status = resp.data.ok_or_else(|| anyhow::anyhow!("Missing data in git status response"))?;

        // Convert changed_files to categorized lists
        let mut staged = Vec::new();
        let mut modified = Vec::new();
        let mut untracked = Vec::new();

        for file in status.changed_files {
            match file.status.as_str() {
                "staged" | "added" => staged.push(file.path),
                "modified" | "deleted" | "renamed" => modified.push(file.path),
                "untracked" => untracked.push(file.path),
                _ => modified.push(file.path), // Default to modified for unknown status
            }
        }

        Ok(GitStatusResponse {
            is_repo: true,
            branch: Some(status.branch),
            staged,
            modified,
            untracked,
        })
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

        let url = format!("{}/sync/command", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "command": "test:run",
                "payload": payload
            }))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("start_test", &body);

        // Parse as raw response first, then extract data
        let raw: RawPluginResponse = serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse start_test response: {}. Body: {}", e, body))?;

        // Try to extract TestStartResponse from data, or construct from top-level
        if raw.data.is_null() || raw.data.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            // Data is empty, construct response from top-level fields
            Ok(TestStartResponse {
                success: raw.success,
                message: raw.error.clone(),
            })
        } else {
            // Try to parse data as TestStartResponse
            serde_json::from_value(raw.data.clone())
                .or_else(|_| Ok(TestStartResponse {
                    success: raw.success,
                    message: raw.data.get("message").and_then(|v| v.as_str()).map(|s| s.to_string()),
                }))
        }
    }

    pub async fn get_test_status(&self) -> anyhow::Result<TestStatusResponse> {
        let url = format!("{}/sync/command", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "command": "test:status",
                "payload": {}
            }))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("get_test_status", &body);

        let raw: RawPluginResponse = serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse get_test_status response: {}. Body: {}", e, body))?;

        // Extract TestStatusResponse from data
        if raw.data.is_null() || raw.data.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            Ok(TestStatusResponse {
                in_progress: false,
                complete: !raw.success,
                error: raw.error,
                output: vec![],
                total_messages: 0,
            })
        } else {
            serde_json::from_value(raw.data.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse TestStatusResponse: {}. Data: {}", e, raw.data))
        }
    }

    pub async fn finish_test(&self) -> anyhow::Result<TestFinishResponse> {
        let url = format!("{}/sync/command", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "command": "test:finish",
                "payload": {}
            }))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("finish_test", &body);

        let raw: RawPluginResponse = serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse finish_test response: {}. Body: {}", e, body))?;

        if raw.data.is_null() || raw.data.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            Ok(TestFinishResponse {
                success: raw.success,
                duration: None,
                output: vec![],
                total_messages: 0,
                error: raw.error,
            })
        } else {
            serde_json::from_value(raw.data.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse TestFinishResponse: {}. Data: {}", e, raw.data))
        }
    }

    pub async fn stop_test(&self) -> anyhow::Result<TestStopResponse> {
        let resp: TestStopResponse = self
            .client
            .post(format!("{}/test/stop", self.base_url))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
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
        let url = format!("{}/bot/observe", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "type": observe_type,
                "radius": radius,
                "query": query
            }))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("bot_observe", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse bot_observe response: {}. Body: {}", e, body))
    }

    /// Move character to position or object
    pub async fn bot_move(
        &self,
        position: Option<serde_json::Value>,
        object_name: Option<&str>,
    ) -> anyhow::Result<BotCommandResponse> {
        let url = format!("{}/bot/move", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "position": position,
                "objectName": object_name
            }))
            .timeout(std::time::Duration::from_secs(60)) // Longer timeout for movement
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("bot_move", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse bot_move response: {}. Body: {}", e, body))
    }

    /// Perform character action
    pub async fn bot_action(
        &self,
        action: &str,
        name: Option<&str>,
    ) -> anyhow::Result<BotCommandResponse> {
        let url = format!("{}/bot/action", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "action": action,
                "name": name
            }))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("bot_action", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse bot_action response: {}. Body: {}", e, body))
    }

    /// Send generic bot command
    pub async fn bot_command(
        &self,
        command_type: &str,
        command: &str,
        args: Option<serde_json::Value>,
    ) -> anyhow::Result<BotCommandResponse> {
        let url = format!("{}/bot/command", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "type": command_type,
                "command": command,
                "args": args.unwrap_or(serde_json::json!({}))
            }))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("bot_command", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse bot_command response: {}. Body: {}", e, body))
    }

    /// Execute Luau code on the server during playtest
    pub async fn bot_query_server(
        &self,
        code: &str,
    ) -> anyhow::Result<BotCommandResponse> {
        let url = format!("{}/bot/query-server", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "code": code
            }))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("bot_query_server", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse bot_query_server response: {}. Body: {}", e, body))
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
        let url = format!("{}/harness/init", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "projectDir": project_dir,
                "gameName": game_name,
                "description": description,
                "genre": genre
            }))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("harness_init", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse harness_init response: {}. Body: {}", e, body))
    }

    /// Start a new development session
    pub async fn harness_session_start(
        &self,
        project_dir: &str,
        initial_goals: Option<&str>,
    ) -> anyhow::Result<SessionStartResponse> {
        let url = format!("{}/harness/session/start", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "projectDir": project_dir,
                "initialGoals": initial_goals
            }))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("harness_session_start", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse harness_session_start response: {}. Body: {}", e, body))
    }

    /// End a development session
    pub async fn harness_session_end(
        &self,
        project_dir: &str,
        session_id: &str,
        summary: Option<&str>,
        handoff_notes: Option<&[String]>,
    ) -> anyhow::Result<SessionEndResponse> {
        let url = format!("{}/harness/session/end", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "projectDir": project_dir,
                "sessionId": session_id,
                "summary": summary,
                "handoffNotes": handoff_notes
            }))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("harness_session_end", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse harness_session_end response: {}. Body: {}", e, body))
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
        let url = format!("{}/harness/feature/update", self.base_url);
        let response = self
            .client
            .post(&url)
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
            .await?;

        let body = response.text().await?;
        debug_log_response("harness_feature_update", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse harness_feature_update response: {}. Body: {}", e, body))
    }

    /// Get harness status for a project
    pub async fn harness_status(
        &self,
        project_dir: &str,
    ) -> anyhow::Result<HarnessStatusResponse> {
        let url = format!("{}/harness/status", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "projectDir": project_dir
            }))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("harness_status", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse harness_status response: {}. Body: {}", e, body))
    }

    /// Read properties of an instance at the given path
    pub async fn read_properties(&self, path: &str) -> anyhow::Result<ReadPropertiesResponse> {
        let url = format!("{}/read-properties", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "path": path
            }))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("read_properties", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse read_properties response: {}. Body: {}", e, body))
    }

    /// Explore the game hierarchy
    pub async fn explore_hierarchy(
        &self,
        path: Option<&str>,
        depth: Option<u32>,
    ) -> anyhow::Result<ExploreHierarchyResponse> {
        let url = format!("{}/explore-hierarchy", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "path": path,
                "depth": depth.unwrap_or(1).min(10)
            }))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("explore_hierarchy", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse explore_hierarchy response: {}. Body: {}", e, body))
    }

    /// Find instances matching search criteria
    pub async fn find_instances(
        &self,
        class_name: Option<&str>,
        name: Option<&str>,
        parent: Option<&str>,
        limit: Option<u32>,
    ) -> anyhow::Result<FindInstancesResponse> {
        let url = format!("{}/find-instances", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "className": class_name,
                "name": name,
                "parent": parent,
                "limit": limit.unwrap_or(100).min(1000)
            }))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;

        let body = response.text().await?;
        debug_log_response("find_instances", &body);

        serde_json::from_str(&body)
            .map_err(|e| anyhow::anyhow!("Failed to parse find_instances response: {}. Body: {}", e, body))
    }

    /// Insert a model from the Roblox marketplace by asset ID
    pub async fn insert_model(
        &self,
        asset_id: u64,
        parent: Option<&str>,
    ) -> anyhow::Result<InsertModelResponse> {
        let resp = self
            .client
            .post(format!("{}/insert-model", self.base_url))
            .json(&serde_json::json!({
                "assetId": asset_id,
                "parent": parent
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
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default, alias = "harnessDir")]
    pub harness_dir: String,
    #[serde(default, alias = "gameId")]
    pub game_id: Option<String>,
}

/// Response from session start
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default, alias = "sessionId")]
    pub session_id: Option<String>,
    #[serde(default, alias = "sessionPath")]
    pub session_path: Option<String>,
}

/// Response from session end
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEndResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub message: String,
}

/// Response from feature update
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureUpdateResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default, alias = "featureId")]
    pub feature_id: Option<String>,
}

/// Summary of feature statuses
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSummary {
    #[serde(default)]
    pub total: usize,
    #[serde(default)]
    pub planned: usize,
    #[serde(default)]
    pub in_progress: usize,
    #[serde(default)]
    pub completed: usize,
    #[serde(default)]
    pub blocked: usize,
    #[serde(default)]
    pub cancelled: usize,
}

/// Brief session summary
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub features_count: usize,
}

/// Response with harness status
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessStatusResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub initialized: bool,
    #[serde(default)]
    pub game: Option<serde_json::Value>,
    #[serde(default)]
    pub features: Vec<serde_json::Value>,
    #[serde(default)]
    pub feature_summary: FeatureSummary,
    #[serde(default)]
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
