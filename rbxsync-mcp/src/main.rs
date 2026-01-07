use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ErrorData as McpError, *},
    schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt,
    transport::stdio,
};
use serde::Deserialize;
use std::borrow::Cow;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod tools;
use tools::RbxSyncClient;

/// RbxSync MCP Server - provides tools for extracting and syncing Roblox games
#[derive(Debug, Clone)]
pub struct RbxSyncServer {
    client: RbxSyncClient,
    tool_router: ToolRouter<RbxSyncServer>,
}

/// Parameters for extract_game tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExtractParams {
    /// The directory where the project files will be written
    #[schemars(description = "The directory where project files will be written")]
    pub project_dir: String,
    /// Optional list of services to extract (e.g., ["Workspace", "ServerScriptService"])
    #[schemars(description = "Optional services to extract")]
    pub services: Option<Vec<String>>,
}

/// Parameters for sync_to_studio tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SyncParams {
    /// The directory containing the project files to sync
    #[schemars(description = "Directory containing project files to sync")]
    pub project_dir: String,
}

/// Parameters for git_commit tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitCommitParams {
    /// The project directory
    #[schemars(description = "The project directory")]
    pub project_dir: String,
    /// The commit message
    #[schemars(description = "The commit message")]
    pub message: String,
    /// Optional list of specific files to commit
    #[schemars(description = "Optional files to commit")]
    pub files: Option<Vec<String>>,
}

/// Parameters for git_status tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitStatusParams {
    /// The project directory
    #[schemars(description = "The project directory")]
    pub project_dir: String,
}

/// Parameters for run_code tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunCodeParams {
    /// Luau code to execute in Roblox Studio
    #[schemars(description = "Luau code to execute in Studio")]
    pub code: String,
}

/// Parameters for run_test tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunTestParams {
    /// How long to run the test in seconds (default: 5)
    #[schemars(description = "Test duration in seconds (default: 5)")]
    pub duration: Option<u32>,
    /// Test mode: "Play" for solo play, "Run" for server simulation (default: "Play")
    #[schemars(description = "Test mode: Play or Run (default: Play)")]
    pub mode: Option<String>,
}

fn mcp_error(msg: impl Into<String>) -> McpError {
    McpError {
        code: ErrorCode(-32603),
        message: Cow::from(msg.into()),
        data: None,
    }
}

#[tool_router]
impl RbxSyncServer {
    pub fn new() -> Self {
        Self {
            client: RbxSyncClient::new(44755),
            tool_router: Self::tool_router(),
        }
    }

    /// Extract a Roblox game from Studio to git-friendly files on disk.
    #[tool(description = "Extract a Roblox game from Studio to git-friendly files")]
    async fn extract_game(
        &self,
        Parameters(params): Parameters<ExtractParams>,
    ) -> Result<CallToolResult, McpError> {
        // Check connection
        let health = self.client.check_health().await.map_err(|e| mcp_error(e.to_string()))?;

        if !health {
            return Ok(CallToolResult::success(vec![Content::text(
                "Error: Not connected to RbxSync server. Make sure 'rbxsync serve' is running and Studio plugin is active.",
            )]));
        }

        // Start extraction
        let session = self.client
            .start_extraction(&params.project_dir, params.services.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        // Poll for completion
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let status = self.client.get_extraction_status().await.map_err(|e| mcp_error(e.to_string()))?;

            match status.status.as_str() {
                "complete" => break,
                "error" => {
                    return Ok(CallToolResult::success(vec![Content::text(format!(
                        "Extraction error: {}",
                        status.error.unwrap_or_default()
                    ))]));
                }
                _ => continue,
            }
        }

        // Finalize extraction
        let result = self.client
            .finalize_extraction(&session.session_id, &params.project_dir)
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Successfully extracted game to {}. {} files written.",
            params.project_dir, result.files_written
        ))]))
    }

    /// Sync local file changes back to Roblox Studio.
    #[tool(description = "Sync local file changes back to Roblox Studio")]
    async fn sync_to_studio(
        &self,
        Parameters(params): Parameters<SyncParams>,
    ) -> Result<CallToolResult, McpError> {
        // Read the local tree
        let tree = self.client.read_tree(&params.project_dir).await.map_err(|e| mcp_error(e.to_string()))?;

        // Build sync operations
        let operations: Vec<_> = tree
            .instances
            .into_iter()
            .map(|inst| tools::SyncOperation {
                op: "update".to_string(),
                path: inst.path,
                class_name: Some(inst.class_name),
                name: Some(inst.name),
                properties: Some(inst.properties),
                source: inst.source,
            })
            .collect();

        if operations.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text("No changes to sync.")]));
        }

        // Apply changes
        let result = self.client.sync_batch(&operations).await.map_err(|e| mcp_error(e.to_string()))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Successfully synced {} instances to Studio.",
                result.applied
            ))]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Sync completed with errors: {:?}",
                result.errors
            ))]))
        }
    }

    /// Get the git status of a project directory.
    #[tool(description = "Get git status of the project")]
    async fn git_status(
        &self,
        Parameters(params): Parameters<GitStatusParams>,
    ) -> Result<CallToolResult, McpError> {
        let status = self.client.get_git_status(&params.project_dir).await.map_err(|e| mcp_error(e.to_string()))?;

        if !status.is_repo {
            return Ok(CallToolResult::success(vec![Content::text("Not a git repository.")]));
        }

        let mut lines = vec![format!("Branch: {}", status.branch.unwrap_or_default())];

        if !status.staged.is_empty() {
            lines.push(format!("Staged ({}):", status.staged.len()));
            for f in &status.staged {
                lines.push(format!("  + {}", f));
            }
        }

        if !status.modified.is_empty() {
            lines.push(format!("Modified ({}):", status.modified.len()));
            for f in &status.modified {
                lines.push(format!("  ~ {}", f));
            }
        }

        if !status.untracked.is_empty() {
            lines.push(format!("Untracked ({}):", status.untracked.len()));
            for f in &status.untracked {
                lines.push(format!("  ? {}", f));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(lines.join("\n"))]))
    }

    /// Commit changes to git.
    #[tool(description = "Commit changes to git")]
    async fn git_commit(
        &self,
        Parameters(params): Parameters<GitCommitParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .git_commit(&params.project_dir, &params.message, params.files.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Committed: {}",
                result.hash.unwrap_or_default()
            ))]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Commit failed: {}",
                result.error.unwrap_or_default()
            ))]))
        }
    }

    /// Execute Luau code in Roblox Studio.
    #[tool(description = "Execute Luau code in Roblox Studio")]
    async fn run_code(
        &self,
        Parameters(params): Parameters<RunCodeParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client.run_code(&params.code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Run an automated play test in Roblox Studio and capture console output.
    /// Starts a play session, captures all prints/warnings/errors, then stops and returns output.
    #[tool(description = "Run automated play test in Studio and return console output")]
    async fn run_test(
        &self,
        Parameters(params): Parameters<RunTestParams>,
    ) -> Result<CallToolResult, McpError> {
        // Start the test
        let start_result = self.client
            .start_test(params.duration, params.mode.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !start_result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to start test: {}",
                start_result.message.unwrap_or_default()
            ))]));
        }

        // Wait for test to complete (poll status)
        let duration_secs = params.duration.unwrap_or(5);
        let poll_interval = tokio::time::Duration::from_millis(500);
        let max_wait = tokio::time::Duration::from_secs((duration_secs + 5) as u64);
        let start_time = tokio::time::Instant::now();

        loop {
            tokio::time::sleep(poll_interval).await;

            let status = self.client.get_test_status().await.map_err(|e| mcp_error(e.to_string()))?;

            if status.complete || !status.in_progress {
                break;
            }

            if start_time.elapsed() > max_wait {
                break;
            }
        }

        // Finish and get results
        let result = self.client.finish_test().await.map_err(|e| mcp_error(e.to_string()))?;

        // Format output
        let mut output_lines = vec![
            format!("Test completed in {:.1}s", result.duration.unwrap_or(0.0)),
            format!("Total messages: {}", result.total_messages),
            String::new(),
        ];

        // Group by message type
        let errors: Vec<_> = result.output.iter().filter(|m| m.msg_type == "MessageError").collect();
        let warnings: Vec<_> = result.output.iter().filter(|m| m.msg_type == "MessageWarning").collect();
        let prints: Vec<_> = result.output.iter().filter(|m| m.msg_type == "MessageOutput").collect();

        if !errors.is_empty() {
            output_lines.push(format!("=== ERRORS ({}) ===", errors.len()));
            for msg in errors {
                output_lines.push(format!("[{:.2}s] {}", msg.timestamp, msg.message));
            }
            output_lines.push(String::new());
        }

        if !warnings.is_empty() {
            output_lines.push(format!("=== WARNINGS ({}) ===", warnings.len()));
            for msg in warnings {
                output_lines.push(format!("[{:.2}s] {}", msg.timestamp, msg.message));
            }
            output_lines.push(String::new());
        }

        if !prints.is_empty() {
            output_lines.push(format!("=== OUTPUT ({}) ===", prints.len()));
            for msg in prints {
                output_lines.push(format!("[{:.2}s] {}", msg.timestamp, msg.message));
            }
        }

        if let Some(err) = result.error {
            output_lines.insert(0, format!("Test error: {}", err));
        }

        Ok(CallToolResult::success(vec![Content::text(output_lines.join("\n"))]))
    }
}

#[tool_handler]
impl ServerHandler for RbxSyncServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "RbxSync MCP Server - Extract and sync Roblox games with git integration. \
                 Requires 'rbxsync serve' running and the RbxSync Studio plugin installed."
                    .to_string(),
            ),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set up logging to stderr (stdio is for MCP protocol)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false),
        )
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting RbxSync MCP server...");

    let service = RbxSyncServer::new().serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}
