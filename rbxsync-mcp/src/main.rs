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
    /// Whether to include terrain data (voxel chunks). Defaults to true.
    #[schemars(description = "Include terrain data (default: true)")]
    #[serde(default = "default_include_terrain")]
    pub include_terrain: bool,
}

fn default_include_terrain() -> bool {
    true
}

/// Parameters for sync_to_studio tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SyncParams {
    /// The directory containing the project files to sync
    #[schemars(description = "Directory containing project files to sync")]
    pub project_dir: String,

    /// If true, delete instances in Studio that don't exist in local files
    #[schemars(description = "Delete orphaned instances in Studio (optional, default: false)")]
    pub delete: Option<bool>,
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
    /// If true, start the test and return immediately without waiting for completion.
    /// Use this for interactive bot testing with bot_observe/bot_move/bot_action.
    #[schemars(description = "Run in background mode - start test and return immediately (default: false)")]
    pub background: Option<bool>,
}

/// Parameters for insert_model tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InsertModelParams {
    /// Roblox asset ID to insert
    #[schemars(description = "Roblox asset ID (number) of the model to insert")]
    #[serde(rename = "assetId")]
    pub asset_id: u64,
    /// Parent path to insert the model into (e.g., "Workspace", "ServerStorage/Items")
    #[schemars(description = "Parent path to insert into (default: Workspace)")]
    pub parent: Option<String>,
}

// ============================================================================
// Bot Controller Parameters (AI-powered automated gameplay testing)
// ============================================================================

/// Parameters for bot_observe tool - get current game state
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BotObserveParams {
    /// Type of observation: "state", "nearby", "npcs", "inventory", "find"
    #[schemars(description = "Observation type: state (full), nearby (objects), npcs, inventory, find (search)")]
    #[serde(rename = "type", default = "default_observe_type")]
    pub observe_type: String,
    /// Radius for nearby/npcs observations (default: 50 studs)
    #[schemars(description = "Search radius in studs (for nearby/npcs)")]
    pub radius: Option<f64>,
    /// Query string for find observations
    #[schemars(description = "Search query (for find type)")]
    pub query: Option<String>,
}

fn default_observe_type() -> String {
    "state".to_string()
}

/// Parameters for bot_move tool - move character to a position or object
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BotMoveParams {
    /// Target position as {x, y, z}
    #[schemars(description = "Target position {x, y, z} - use this OR objectName")]
    pub position: Option<serde_json::Value>,
    /// Name of object to move to
    #[schemars(description = "Name of object to navigate to - use this OR position")]
    #[serde(rename = "objectName")]
    pub object_name: Option<String>,
}

/// Parameters for bot_action tool - perform character actions
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BotActionParams {
    /// Action type: equip, unequip, activate, deactivate, interact, jump
    #[schemars(description = "Action: equip, unequip, activate, deactivate, interact, jump")]
    pub action: String,
    /// Name of tool/object (for equip, interact)
    #[schemars(description = "Tool or object name (for equip, interact actions)")]
    pub name: Option<String>,
}

/// Parameters for bot_command tool - send generic bot command
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BotCommandParams {
    /// Command type: move, action, ui, observe
    #[schemars(description = "Command type: move, action, ui, observe")]
    #[serde(rename = "type")]
    pub command_type: String,
    /// Specific command within the type
    #[schemars(description = "Command name (e.g., moveTo, equipTool, clickButton)")]
    pub command: String,
    /// Command arguments
    #[schemars(description = "Command arguments as JSON object")]
    pub args: Option<serde_json::Value>,
}

/// Parameters for bot_query_server tool - execute Luau code on server during playtest
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BotQueryServerParams {
    /// Luau code to execute on the server. Can be an expression (returns value) or statement.
    /// Examples: "#game.Players:GetPlayers()" returns player count,
    /// "game.Players:GetPlayers()[1].leaderstats.Coins.Value" returns currency
    #[schemars(description = "Luau code to execute on server during playtest")]
    pub code: String,
}

/// Parameters for bot_wait_for tool - wait for a condition during playtest
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BotWaitForParams {
    /// Luau code that returns a boolean. Polling continues until this returns true.
    /// Example: "workspace:FindFirstChild('Ball') == nil" waits until Ball is removed
    #[schemars(description = "Luau condition code that returns true when condition is met")]
    pub condition: String,
    /// Maximum time to wait in seconds (default: 30)
    #[schemars(description = "Timeout in seconds (default: 30)")]
    pub timeout: Option<f64>,
    /// Polling interval in milliseconds (default: 100)
    #[schemars(description = "Poll interval in ms (default: 100)")]
    pub poll_interval: Option<u32>,
    /// Where to evaluate: "server" for server-side state, "client" for client-side (default: server)
    #[schemars(description = "Execution context: 'server' or 'client' (default: server)")]
    pub context: Option<String>,
}

// ============================================================================
// Harness Parameters (Multi-session AI game development tracking)
// ============================================================================

/// Parameters for harness_init tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HarnessInitParams {
    /// The project directory where harness will be initialized
    #[schemars(description = "Project directory path")]
    pub project_dir: String,
    /// Name of the game being developed
    #[schemars(description = "Game name")]
    pub game_name: String,
    /// Optional game description
    #[schemars(description = "Optional game description")]
    pub description: Option<String>,
    /// Optional game genre (e.g., "Obby", "Tycoon", "Simulator")
    #[schemars(description = "Optional game genre")]
    pub genre: Option<String>,
}

/// Parameters for harness_session_start tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HarnessSessionStartParams {
    /// The project directory
    #[schemars(description = "Project directory path")]
    pub project_dir: String,
    /// Optional initial goals for this development session
    #[schemars(description = "Initial goals/focus for this session")]
    pub initial_goals: Option<String>,
}

/// Parameters for harness_session_end tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HarnessSessionEndParams {
    /// The project directory
    #[schemars(description = "Project directory path")]
    pub project_dir: String,
    /// Session ID to end
    #[schemars(description = "Session ID to end")]
    pub session_id: String,
    /// Summary of what was accomplished
    #[schemars(description = "Summary of accomplishments")]
    pub summary: Option<String>,
    /// Notes for the next session/developer
    #[schemars(description = "Handoff notes for future sessions")]
    pub handoff_notes: Option<Vec<String>>,
}

/// Parameters for harness_feature_update tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HarnessFeatureUpdateParams {
    /// The project directory
    #[schemars(description = "Project directory path")]
    pub project_dir: String,
    /// Feature ID (if updating existing feature)
    #[schemars(description = "Feature ID for updates (omit for new features)")]
    pub feature_id: Option<String>,
    /// Feature name (required for new features)
    #[schemars(description = "Feature name (required for new features)")]
    pub name: Option<String>,
    /// Feature description
    #[schemars(description = "Feature description")]
    pub description: Option<String>,
    /// Feature status: planned, in_progress, completed, blocked, cancelled
    #[schemars(description = "Status: planned, in_progress, completed, blocked, cancelled")]
    pub status: Option<String>,
    /// Priority: low, medium, high, critical
    #[schemars(description = "Priority: low, medium, high, critical")]
    pub priority: Option<String>,
    /// Tags to categorize the feature
    #[schemars(description = "Tags for categorization")]
    pub tags: Option<Vec<String>>,
    /// Note to add to the feature
    #[schemars(description = "Note to add")]
    pub add_note: Option<String>,
    /// Session ID working on this feature
    #[schemars(description = "Session ID working on feature")]
    pub session_id: Option<String>,
}

/// Parameters for harness_status tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HarnessStatusParams {
    /// The project directory
    #[schemars(description = "Project directory path")]
    pub project_dir: String,
}

/// Parameters for read_properties tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadPropertiesParams {
    /// Instance path in the hierarchy (e.g., "Workspace/SpawnLocation" or "ServerScriptService/MyScript")
    #[schemars(description = "Instance path (e.g., 'Workspace/SpawnLocation')")]
    pub path: String,
}

/// Parameters for explore_hierarchy tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExploreHierarchyParams {
    /// Starting path in the hierarchy (e.g., "Workspace" or "ServerScriptService/MyFolder").
    /// If not provided, returns top-level services.
    #[schemars(description = "Starting path (e.g., 'Workspace'). Omit for top-level services.")]
    pub path: Option<String>,
    /// Maximum depth to traverse (1 = direct children only, 2 = children and grandchildren, etc.)
    /// Default is 1. Maximum is 10.
    #[schemars(description = "Depth limit (default: 1, max: 10)")]
    pub depth: Option<u32>,
}

/// Parameters for find_instances tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindInstancesParams {
    /// Filter by ClassName (e.g., "Part", "Script", "Model")
    #[schemars(description = "Filter by ClassName (e.g., 'Part', 'Script', 'Model')")]
    #[serde(rename = "className")]
    pub class_name: Option<String>,
    /// Filter by instance Name (supports pattern matching with *)
    #[schemars(description = "Filter by Name (supports * wildcard, e.g., 'Enemy*')")]
    pub name: Option<String>,
    /// Search within a specific path (e.g., "Workspace/Enemies")
    #[schemars(description = "Search within path (e.g., 'Workspace/Enemies'). Omit for entire game.")]
    pub parent: Option<String>,
    /// Maximum number of results to return (default: 100, max: 1000)
    #[schemars(description = "Max results (default: 100, max: 1000)")]
    pub limit: Option<u32>,
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
            .start_extraction(&params.project_dir, params.services.as_deref(), params.include_terrain)
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        // Poll for completion
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let status = self.client.get_extraction_status().await.map_err(|e| mcp_error(e.to_string()))?;

            if status.complete {
                break;
            }
            if let Some(err) = &status.error {
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Extraction error: {}",
                    err
                ))]));
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
        // Use incremental sync - only reads files modified since last sync
        let incremental = self.client.read_incremental(&params.project_dir).await.map_err(|e| mcp_error(e.to_string()))?;

        // Build sync operations in the format expected by the plugin
        let mut operations = tools::build_sync_operations(incremental.instances);

        // If delete flag is set, add delete operations for orphaned instances
        let delete_count = if params.delete.unwrap_or(false) {
            let diff = self.client.get_diff(&params.project_dir).await.map_err(|e| mcp_error(e.to_string()))?;
            let removed_count = diff.removed.len();
            for entry in diff.removed {
                operations.push(serde_json::json!({
                    "type": "delete",
                    "path": entry.path
                }));
            }
            removed_count
        } else {
            0
        };

        if operations.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text("No changes to sync.")]));
        }

        // Apply changes (pass project_dir for operation tracking - RBXSYNC-77)
        let result = self.client.sync_batch(&operations, Some(&params.project_dir)).await.map_err(|e| mcp_error(e.to_string()))?;

        // Check if sync was skipped (disabled or extraction in progress)
        if let Some(ref data) = result.data {
            if let Some(ref reason) = data.reason {
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Sync skipped: {}. Enable 'Files → Studio' in the RbxSync plugin or wait for extraction to complete.",
                    reason
                ))]));
            }
        }

        // Extract applied count from nested data or top-level field
        let applied = result.data.as_ref().map(|d| d.applied).unwrap_or(result.applied);
        let errors = result.data.as_ref().map(|d| d.errors.clone()).unwrap_or(result.errors);

        if result.success && errors.is_empty() {
            // Mark as synced for next incremental sync
            let _ = self.client.mark_synced(&params.project_dir).await;

            let sync_type = if incremental.full_sync { "full" } else { "incremental" };
            let msg = if delete_count > 0 {
                format!(
                    "Successfully synced {} instances ({} sync, checked {} files) and deleted {} orphans.",
                    applied, sync_type, incremental.files_checked, delete_count
                )
            } else {
                format!(
                    "Successfully synced {} instances to Studio ({} sync, {} of {} files modified).",
                    applied, sync_type, incremental.files_modified, incremental.files_checked
                )
            };
            Ok(CallToolResult::success(vec![Content::text(msg)]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Sync completed with errors: {:?}",
                errors
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
    /// For interactive bot testing, use background: true to start the test and return immediately,
    /// then use bot_observe/bot_move/bot_action while the test runs.
    /// IMPORTANT: Stop playtest with stop_test before making code changes.
    /// Changes won't take effect until you stop_test, sync, then run_test again.
    #[tool(description = "Run automated play test in Studio and return console output. For interactive bot testing, use background: true to start test and return immediately. IMPORTANT: Stop playtest with stop_test before making code changes.")]
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

        // Background mode: return immediately after starting the test
        if params.background.unwrap_or(false) {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "started": true,
                    "mode": params.mode.as_deref().unwrap_or("Play"),
                    "message": "Test started in background. Use bot_observe/bot_move/bot_action to interact."
                }).to_string()
            )]));
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

    /// Stop any running playtest in Roblox Studio.
    /// Call this before making code changes - changes won't take effect until you stop the test,
    /// sync your changes, then run a new test.
    #[tool(description = "Stop any running playtest. Call before making code changes.")]
    async fn stop_test(&self) -> Result<CallToolResult, McpError> {
        let result = self.client.stop_test().await.map_err(|e| mcp_error(e.to_string()))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(
                result.message.unwrap_or_else(|| "Playtest stopped successfully.".to_string())
            )]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to stop playtest: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            ))]))
        }
    }

    // ========================================================================
    // Bot Controller Tools (AI-powered automated gameplay testing)
    // ========================================================================

    /// Observe current game state during a playtest.
    /// Returns character position, health, inventory, nearby objects/NPCs, and visible UI.
    /// Must be called during an active playtest (after run_test or manual F5).
    #[tool(description = "Observe game state during playtest - get position, health, inventory, nearby objects")]
    async fn bot_observe(
        &self,
        Parameters(params): Parameters<BotObserveParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .bot_observe(&params.observe_type, params.radius, params.query.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Observation failed: {}",
                result.error.unwrap_or_default()
            ))]));
        }

        // Format the state nicely
        let state_json = serde_json::to_string_pretty(&result.data)
            .unwrap_or_else(|_| "{}".to_string());

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Game State:\n{}",
            state_json
        ))]))
    }

    /// Move character to a position or named object using pathfinding.
    /// The character will navigate around obstacles using PathfindingService.
    /// Must be called during an active playtest.
    #[tool(description = "Move character to position {x,y,z} or object name using pathfinding")]
    async fn bot_move(
        &self,
        Parameters(params): Parameters<BotMoveParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .bot_move(params.position, params.object_name.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Movement failed: {}",
                result.error.unwrap_or_default()
            ))]));
        }

        let reached = result.data.as_ref()
            .and_then(|d| d.get("reached"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let final_pos = result.data.as_ref()
            .and_then(|d| d.get("finalPosition"))
            .map(|v| format!("{}", v))
            .unwrap_or_default();

        if reached {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Successfully reached destination. Final position: {}",
                final_pos
            ))]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Movement completed but may not have reached exact destination. Final position: {}. Error: {}",
                final_pos,
                result.data.as_ref().and_then(|d| d.get("error")).and_then(|e| e.as_str()).unwrap_or("none")
            ))]))
        }
    }

    /// Perform character actions: equip/unequip tools, activate abilities, interact with objects.
    /// Actions: equip, unequip, activate, deactivate, interact, jump
    /// Must be called during an active playtest.
    #[tool(description = "Perform actions: equip/unequip tools, activate, interact with objects, jump")]
    async fn bot_action(
        &self,
        Parameters(params): Parameters<BotActionParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .bot_action(&params.action, params.name.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Action '{}' failed: {}",
                params.action,
                result.error.unwrap_or_default()
            ))]));
        }

        let action_result = result.data.as_ref()
            .and_then(|d| d.get("result"))
            .map(|v| format!("{}", v))
            .unwrap_or_else(|| "completed".to_string());

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Action '{}' completed: {}",
            params.action,
            action_result
        ))]))
    }

    /// Send a generic bot command for advanced control.
    /// Supports movement, actions, UI interactions, and observations.
    /// Must be called during an active playtest.
    #[tool(description = "Send generic bot command for advanced character control")]
    async fn bot_command(
        &self,
        Parameters(params): Parameters<BotCommandParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .bot_command(&params.command_type, &params.command, params.args.clone())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Command '{}.{}' failed: {}",
                params.command_type,
                params.command,
                result.error.unwrap_or_default()
            ))]));
        }

        let result_json = serde_json::to_string_pretty(&result.data)
            .unwrap_or_else(|_| "{}".to_string());

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Command '{}.{}' result:\n{}",
            params.command_type,
            params.command,
            result_json
        ))]))
    }

    /// Execute Luau code on the game server during an active playtest.
    /// Use this to query game state that only exists on the server (currency, DataStores, services).
    /// Returns the result of the code execution.
    /// Must be called during an active playtest.
    #[tool(description = "Execute Luau code on game server during playtest - query currency, DataStores, services")]
    async fn bot_query_server(
        &self,
        Parameters(params): Parameters<BotQueryServerParams>,
    ) -> Result<CallToolResult, McpError> {
        // Send as a dedicated bot query server command
        let result = self.client
            .bot_query_server(&params.code)
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Server query failed: {}",
                result.error.unwrap_or_default()
            ))]));
        }

        // Extract the result from the response
        let query_result = result.data.as_ref()
            .and_then(|d| d.get("result"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let context = result.data.as_ref()
            .and_then(|d| d.get("context"))
            .and_then(|c| c.as_str())
            .unwrap_or("unknown");

        let result_str = serde_json::to_string_pretty(&query_result)
            .unwrap_or_else(|_| format!("{:?}", query_result));

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Server query result (context: {}):\n{}",
            context,
            result_str
        ))]))
    }

    /// Wait for a condition to become true during an active playtest.
    /// Polls the condition at regular intervals until it returns true or timeout.
    /// Use context "server" for server-side state, "client" for client-side.
    /// Must be called during an active playtest.
    #[tool(description = "Wait for a Luau condition to become true during playtest")]
    async fn bot_wait_for(
        &self,
        Parameters(params): Parameters<BotWaitForParams>,
    ) -> Result<CallToolResult, McpError> {
        let context = params.context.as_deref().unwrap_or("server");
        let command = if context == "server" { "waitForServer" } else { "waitFor" };

        let result = self.client
            .bot_command("query", command, Some(serde_json::json!({
                "condition": params.condition,
                "timeout": params.timeout.unwrap_or(30.0),
                "pollInterval": params.poll_interval.unwrap_or(100)
            })))
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Wait failed: {}",
                result.error.unwrap_or_default()
            ))]));
        }

        let condition_met = result.data.as_ref()
            .and_then(|d| d.get("result"))
            .and_then(|r| r.as_bool())
            .unwrap_or(false);

        let timed_out = result.data.as_ref()
            .and_then(|d| d.get("timedOut"))
            .and_then(|t| t.as_bool())
            .unwrap_or(false);

        let elapsed = result.data.as_ref()
            .and_then(|d| d.get("elapsed"))
            .and_then(|e| e.as_f64())
            .unwrap_or(0.0);

        if condition_met {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Condition met after {:.2}s",
                elapsed
            ))]))
        } else if timed_out {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Condition NOT met - timed out after {:.2}s",
                elapsed
            ))]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Wait completed in {:.2}s, result: {:?}",
                elapsed,
                result.data
            ))]))
        }
    }

    // ========================================================================
    // Harness Tools (Multi-session AI game development tracking)
    // ========================================================================

    /// Initialize harness for a project.
    /// Creates the .rbxsync/harness directory structure with game.yaml and features.yaml.
    /// Call this once at the start of a new game project.
    #[tool(description = "Initialize harness for a project")]
    async fn harness_init(
        &self,
        Parameters(params): Parameters<HarnessInitParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .harness_init(
                &params.project_dir,
                &params.game_name,
                params.description.as_deref(),
                params.genre.as_deref(),
            )
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Harness initialized at {}. Game ID: {}",
                result.harness_dir,
                result.game_id.unwrap_or_default()
            ))]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to initialize harness: {}",
                result.message
            ))]))
        }
    }

    /// Start a new development session.
    /// Creates a session log to track work done across this conversation.
    /// Returns a session ID that can be used to end the session later.
    #[tool(description = "Start dev session, get context")]
    async fn harness_session_start(
        &self,
        Parameters(params): Parameters<HarnessSessionStartParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .harness_session_start(&params.project_dir, params.initial_goals.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Session started. ID: {}\nPath: {}",
                result.session_id.unwrap_or_default(),
                result.session_path.unwrap_or_default()
            ))]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to start session: {}",
                result.message
            ))]))
        }
    }

    /// End a development session.
    /// Updates the session log with summary and handoff notes for future sessions.
    #[tool(description = "End session with handoff notes")]
    async fn harness_session_end(
        &self,
        Parameters(params): Parameters<HarnessSessionEndParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .harness_session_end(
                &params.project_dir,
                &params.session_id,
                params.summary.as_deref(),
                params.handoff_notes.as_deref(),
            )
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(
                "Session ended successfully."
            )]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to end session: {}",
                result.message
            ))]))
        }
    }

    /// Create or update a feature in the project.
    /// Features track game functionality being developed across sessions.
    /// Provide feature_id to update an existing feature, or name to create a new one.
    #[tool(description = "Create/update feature status")]
    async fn harness_feature_update(
        &self,
        Parameters(params): Parameters<HarnessFeatureUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .harness_feature_update(
                &params.project_dir,
                params.feature_id.as_deref(),
                params.name.as_deref(),
                params.description.as_deref(),
                params.status.as_deref(),
                params.priority.as_deref(),
                params.tags.as_deref(),
                params.add_note.as_deref(),
                params.session_id.as_deref(),
            )
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Feature {}: {}",
                result.feature_id.unwrap_or_default(),
                result.message
            ))]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to update feature: {}",
                result.message
            ))]))
        }
    }

    /// Read all properties of an instance at the given path.
    /// Returns className, name, and all serialized properties.
    /// Useful for inspecting instance state without running code.
    #[tool(description = "Read properties of an instance at a path (e.g., 'Workspace/SpawnLocation')")]
    async fn read_properties(
        &self,
        Parameters(params): Parameters<ReadPropertiesParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .read_properties(&params.path)
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to read properties: {}",
                result.error.unwrap_or_default()
            ))]));
        }

        // Format the properties nicely
        let mut output = vec![];

        if let Some(data) = &result.data {
            if let Some(class_name) = data.get("className").and_then(|v| v.as_str()) {
                output.push(format!("ClassName: {}", class_name));
            }
            if let Some(name) = data.get("name").and_then(|v| v.as_str()) {
                output.push(format!("Name: {}", name));
            }
            if let Some(path) = data.get("path").and_then(|v| v.as_str()) {
                output.push(format!("Path: {}", path));
            }

            output.push(String::new());

            // Show properties
            if let Some(props) = data.get("properties") {
                output.push("Properties:".to_string());
                let props_json = serde_json::to_string_pretty(props)
                    .unwrap_or_else(|_| "{}".to_string());
                output.push(props_json);
            }

            // Show attributes if present
            if let Some(attrs) = data.get("attributes") {
                if !attrs.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                    output.push(String::new());
                    output.push("Attributes:".to_string());
                    let attrs_json = serde_json::to_string_pretty(attrs)
                        .unwrap_or_else(|_| "{}".to_string());
                    output.push(attrs_json);
                }
            }

            // Show tags if present
            if let Some(tags) = data.get("tags") {
                if !tags.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                    output.push(String::new());
                    output.push(format!("Tags: {:?}", tags));
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(output.join("\n"))]))
    }

    /// Explore the game hierarchy to discover instances.
    /// Returns a tree of instances with their className, name, and childCount.
    /// Use path to start from a specific location, or omit for top-level services.
    /// Use depth to control how deep to traverse (default 1).
    #[tool(description = "Explore game hierarchy - returns tree of instances with className and childCount")]
    async fn explore_hierarchy(
        &self,
        Parameters(params): Parameters<ExploreHierarchyParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .explore_hierarchy(params.path.as_deref(), params.depth)
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to explore hierarchy: {}",
                result.error.unwrap_or_default()
            ))]));
        }

        // Format the tree nicely
        fn format_node(node: &serde_json::Value, indent: usize) -> String {
            let mut lines = vec![];
            let prefix = "  ".repeat(indent);

            let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let class_name = node.get("className").and_then(|v| v.as_str()).unwrap_or("?");
            let child_count = node.get("childCount").and_then(|v| v.as_u64()).unwrap_or(0);

            let children = node.get("children").and_then(|v| v.as_array());

            if let Some(children) = children {
                if children.is_empty() {
                    lines.push(format!("{}{} [{}]", prefix, name, class_name));
                } else {
                    lines.push(format!("{}{} [{}] ({} children)", prefix, name, class_name, child_count));
                    for child in children {
                        lines.push(format_node(child, indent + 1));
                    }
                }
            } else if child_count > 0 {
                // Has children but not expanded (depth limit reached)
                lines.push(format!("{}{} [{}] ({} children...)", prefix, name, class_name, child_count));
            } else {
                lines.push(format!("{}{} [{}]", prefix, name, class_name));
            }

            lines.join("\n")
        }

        let mut output = vec![];

        if let Some(data) = &result.data {
            if let Some(tree) = data.as_array() {
                // Multiple root nodes (services)
                for node in tree {
                    output.push(format_node(node, 0));
                }
            } else {
                // Single root node
                output.push(format_node(data, 0));
            }
        }

        if output.is_empty() {
            output.push("No instances found.".to_string());
        }

        Ok(CallToolResult::success(vec![Content::text(output.join("\n"))]))
    }

    /// Find instances matching search criteria.
    /// Searches by className, name pattern, and/or within a specific parent path.
    /// Returns a list of matching instances with their paths.
    #[tool(description = "Find instances by className, name pattern, or parent path")]
    async fn find_instances(
        &self,
        Parameters(params): Parameters<FindInstancesParams>,
    ) -> Result<CallToolResult, McpError> {
        // Require at least one filter
        if params.class_name.is_none() && params.name.is_none() && params.parent.is_none() {
            return Ok(CallToolResult::success(vec![Content::text(
                "Error: At least one filter (className, name, or parent) is required."
            )]));
        }

        let result = self.client
            .find_instances(
                params.class_name.as_deref(),
                params.name.as_deref(),
                params.parent.as_deref(),
                params.limit,
            )
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to find instances: {}",
                result.error.unwrap_or_default()
            ))]));
        }

        // Format results
        let mut output = vec![];

        if let Some(data) = &result.data {
            if let Some(instances) = data.get("instances").and_then(|v| v.as_array()) {
                let total = data.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
                let limited = data.get("limited").and_then(|v| v.as_bool()).unwrap_or(false);

                if instances.is_empty() {
                    output.push("No instances found matching criteria.".to_string());
                } else {
                    if limited {
                        output.push(format!("Found {} instances (showing first {}):", total, instances.len()));
                    } else {
                        output.push(format!("Found {} instances:", instances.len()));
                    }
                    output.push(String::new());

                    for inst in instances {
                        let class_name = inst.get("className").and_then(|v| v.as_str()).unwrap_or("?");
                        let path = inst.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        output.push(format!("  {} [{}]", path, class_name));
                    }
                }
            } else {
                output.push("No results returned.".to_string());
            }
        } else {
            output.push("No data returned.".to_string());
        }

        Ok(CallToolResult::success(vec![Content::text(output.join("\n"))]))
    }

    /// Insert a model from the Roblox marketplace into the game.
    /// Uses InsertService:LoadAsset to fetch the model by asset ID.
    /// Returns the inserted model's name, path, and className.
    #[tool(description = "Insert a Roblox marketplace model by asset ID into Studio")]
    async fn insert_model(
        &self,
        Parameters(params): Parameters<InsertModelParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .insert_model(params.asset_id, params.parent.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to insert model: {}",
                result.error.unwrap_or_default()
            ))]));
        }

        let inserted_name = result.inserted_name.unwrap_or_else(|| "Unknown".to_string());
        let inserted_path = result.inserted_path.unwrap_or_else(|| "Unknown".to_string());
        let class_name = result.class_name.unwrap_or_else(|| "Unknown".to_string());

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Successfully inserted model:\n  Name: {}\n  Path: {}\n  ClassName: {}",
            inserted_name, inserted_path, class_name
        ))]))
    }

    /// Get current harness state for a project.
    /// Returns game info, features list with status summary, and recent sessions.
    #[tool(description = "Get current harness state")]
    async fn harness_status(
        &self,
        Parameters(params): Parameters<HarnessStatusParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .harness_status(&params.project_dir)
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.initialized {
            return Ok(CallToolResult::success(vec![Content::text(
                "Harness not initialized. Use harness_init to set up the project."
            )]));
        }

        let mut output = vec!["=== Harness Status ===".to_string()];

        // Game info
        if let Some(game) = &result.game {
            let name = game.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
            output.push(format!("\nGame: {}", name));
            if let Some(desc) = game.get("description").and_then(|v| v.as_str()) {
                if !desc.is_empty() {
                    output.push(format!("Description: {}", desc));
                }
            }
        }

        // Feature summary
        let summary = &result.feature_summary;
        output.push(format!(
            "\nFeatures: {} total ({} planned, {} in progress, {} completed, {} blocked)",
            summary.total, summary.planned, summary.in_progress, summary.completed, summary.blocked
        ));

        // List features
        if !result.features.is_empty() {
            output.push("\nFeature List:".to_string());
            for feature in &result.features {
                let id = feature.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let name = feature.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed");
                let status = feature.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                output.push(format!("  - [{}] {} ({})", id, name, status));
            }
        }

        // Recent sessions
        if !result.recent_sessions.is_empty() {
            output.push("\nRecent Sessions:".to_string());
            for session in &result.recent_sessions {
                let status = if session.ended_at.is_some() { "ended" } else { "active" };
                output.push(format!(
                    "  - {} ({}, {} features)",
                    session.id, status, session.features_count
                ));
                if !session.summary.is_empty() {
                    output.push(format!("    Summary: {}", session.summary));
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(output.join("\n"))]))
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
