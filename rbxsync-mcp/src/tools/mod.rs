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
    pub connected: bool,
}

#[derive(Debug, Deserialize)]
pub struct ExtractStartResponse {
    pub session_id: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ExtractStatusResponse {
    pub status: String,
    pub current_service: Option<String>,
    pub instances_extracted: i32,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractFinalizeResponse {
    pub success: bool,
    pub files_written: i32,
}

#[derive(Debug, Deserialize)]
pub struct SyncReadTreeResponse {
    pub instances: Vec<InstanceData>,
    pub total_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct InstanceData {
    pub path: String,
    pub class_name: String,
    pub name: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncOperation {
    pub op: String,
    pub path: String,
    pub class_name: Option<String>,
    pub name: Option<String>,
    pub properties: Option<HashMap<String, serde_json::Value>>,
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SyncBatchResponse {
    pub success: bool,
    pub applied: i32,
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

        Ok(resp.connected)
    }

    pub async fn start_extraction(
        &self,
        project_dir: &str,
        services: Option<&[String]>,
    ) -> anyhow::Result<ExtractStartResponse> {
        let mut body = serde_json::json!({
            "project_dir": project_dir
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

    pub async fn sync_batch(&self, operations: &[SyncOperation]) -> anyhow::Result<SyncBatchResponse> {
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
}
