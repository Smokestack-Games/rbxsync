use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ErrorData as McpError, *},
    schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt,
    transport::stdio,
};
use serde::Deserialize;
use std::borrow::Cow;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod luau_helpers;
mod tools;
use luau_helpers::{escape_luau_string, json_value_to_luau, luau_navigate_snippet, validate_luau_identifier};
use tools::RbxSyncClient;

/// RbxSync MCP Server - provides tools for extracting and syncing Roblox games
#[derive(Debug, Clone)]
pub struct RbxSyncServer {
    client: RbxSyncClient,
    tool_router: ToolRouter<RbxSyncServer>,
    active_place_id: std::sync::Arc<tokio::sync::RwLock<Option<u64>>>,
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
    /// Optional Place ID to target a specific Studio instance in multi-place projects
    #[schemars(description = "Place ID to target (optional, for multi-place projects)")]
    #[serde(default)]
    pub place_id: Option<u64>,
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

    /// Optional Place ID to target a specific Studio instance in multi-place projects
    #[schemars(description = "Place ID to target (optional, for multi-place projects)")]
    #[serde(default)]
    pub place_id: Option<u64>,

    /// Optional list of Place IDs for multi-place sync (syncs to each place sequentially)
    #[schemars(description = "Place IDs for multi-place sync (optional, syncs to each place)")]
    #[serde(default)]
    pub place_ids: Option<Vec<u64>>,
}

/// Parameters for diff tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DiffParams {
    /// The project directory to diff
    #[schemars(description = "Project directory to compare local files vs Studio state")]
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

/// Parameters for verify tool (E2E testing)
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VerifyParams {
    /// Check type: property, count, find, children, attribute, distance, backpack, leaderstat
    #[schemars(description = "Check type: property, count, find, children, attribute, distance, backpack, leaderstat")]
    pub check: String,
    /// Instance path (e.g., "Workspace.SpawnLocation")
    #[schemars(description = "Instance path (e.g., 'Workspace.SpawnLocation')")]
    #[serde(default)]
    pub path: Option<String>,
    /// Property name to check
    #[schemars(description = "Property name to check")]
    #[serde(default)]
    pub property: Option<String>,
    /// Comparison operator: eq, ne, gt, lt, gte, lte, contains, matches
    #[schemars(description = "Comparison operator: eq, ne, gt, lt, gte, lte, contains, matches")]
    #[serde(default)]
    pub operator: Option<String>,
    /// Expected value for the check
    #[schemars(description = "Expected value for the check")]
    #[serde(default)]
    pub expected: Option<serde_json::Value>,
    /// Custom message for the check result
    #[schemars(description = "Custom message for the check result")]
    #[serde(default)]
    pub message: Option<String>,
    /// Timeout in seconds for 'eventually true' polling (0 = instant check)
    #[schemars(description = "Timeout in seconds for polling (0 = instant check)")]
    #[serde(default)]
    pub timeout: Option<u32>,
    /// Tag name for CollectionService tag checks
    #[schemars(description = "Tag name for CollectionService tag checks")]
    #[serde(default)]
    pub tag: Option<String>,
    /// Class name filter for count/find checks
    #[schemars(description = "Class name filter for count/find checks")]
    #[serde(default)]
    pub class: Option<String>,
    /// Parent path for scoping count/find checks
    #[schemars(description = "Parent path for scoping count/find checks")]
    #[serde(default)]
    pub parent: Option<String>,
    /// Target path for distance checks
    #[schemars(description = "Target path for distance checks")]
    #[serde(default)]
    pub target: Option<String>,
    /// Item name for backpack checks
    #[schemars(description = "Item name for backpack checks")]
    #[serde(default)]
    pub item: Option<String>,
    /// Stat name for leaderstat checks
    #[schemars(description = "Stat name for leaderstat checks")]
    #[serde(default)]
    pub stat: Option<String>,
}

/// Parameters for run_code tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunCodeParams {
    /// Luau code to execute in Roblox Studio
    #[schemars(description = "Luau code to execute in Studio")]
    pub code: String,
    /// Optional Place ID to target a specific Studio instance in multi-place projects
    #[schemars(description = "Place ID to target (optional, for multi-place projects)")]
    #[serde(default)]
    pub place_id: Option<u64>,
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
    /// Optional Place ID to target a specific Studio instance in multi-place projects
    #[schemars(description = "Place ID to target (optional, for multi-place projects)")]
    #[serde(default)]
    pub place_id: Option<u64>,
}

/// Parameters for start_playtest tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StartPlaytestParams {
    /// Play mode: "Play" for solo play, "Run" for server simulation (default: "Play")
    #[schemars(description = "Play mode: Play (solo) or Run (server sim). Default: Play")]
    pub mode: Option<String>,
}

/// Parameters for stop_playtest tool (no params needed)
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StopPlaytestParams {}

/// Parameters for playtest_status tool (no params needed)
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PlaytestStatusParams {}

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

/// Parameters for get_tags tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTagsParams {
    /// Instance path (e.g., "Workspace/MyPart")
    #[schemars(description = "Instance path (e.g., 'Workspace/MyPart')")]
    pub path: String,
}

/// Parameters for get_attributes tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetAttributesParams {
    /// Instance path (e.g., "Workspace/MyPart")
    #[schemars(description = "Instance path (e.g., 'Workspace/MyPart')")]
    pub path: String,
}

/// Parameters for add_tag tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddTagParams {
    /// Instance path (e.g., "Workspace/MyPart")
    #[schemars(description = "Instance path (e.g., 'Workspace/MyPart')")]
    pub path: String,
    /// Tag to add
    #[schemars(description = "Tag name to add")]
    pub tag: String,
}

/// Parameters for remove_tag tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveTagParams {
    /// Instance path (e.g., "Workspace/MyPart")
    #[schemars(description = "Instance path (e.g., 'Workspace/MyPart')")]
    pub path: String,
    /// Tag to remove
    #[schemars(description = "Tag name to remove")]
    pub tag: String,
}

/// Parameters for get_tagged tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTaggedParams {
    /// Tag to search for
    #[schemars(description = "Tag name to search for")]
    pub tag: String,
    /// Maximum results (default: 100)
    #[schemars(description = "Max results (default: 100)")]
    pub limit: Option<u32>,
}

/// Parameters for set_attribute tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetAttributeParams {
    /// Instance path (e.g., "Workspace/MyPart")
    #[schemars(description = "Instance path (e.g., 'Workspace/MyPart')")]
    pub path: String,
    /// Attribute name
    #[schemars(description = "Attribute name to set")]
    pub name: String,
    /// Attribute value (string, number, boolean)
    #[schemars(description = "Attribute value (string, number, or boolean)")]
    pub value: serde_json::Value,
}

/// Parameters for delete_attribute tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteAttributeParams {
    /// Instance path (e.g., "Workspace/MyPart")
    #[schemars(description = "Instance path (e.g., 'Workspace/MyPart')")]
    pub path: String,
    /// Attribute name to remove
    #[schemars(description = "Attribute name to remove")]
    pub name: String,
}

/// Parameters for get_script_source tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetScriptSourceParams {
    /// Path to the script instance
    #[schemars(description = "Script path (e.g., 'ServerScriptService/Main')")]
    pub path: String,
    /// Optional start line (1-indexed)
    #[schemars(description = "Start line number (1-indexed, optional)")]
    pub start_line: Option<u32>,
    /// Optional end line (1-indexed, inclusive)
    #[schemars(description = "End line number (1-indexed, inclusive, optional)")]
    pub end_line: Option<u32>,
}

/// Parameters for set_script_source tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetScriptSourceParams {
    /// Path to the script instance
    #[schemars(description = "Script path (e.g., 'ServerScriptService/Main')")]
    pub path: String,
    /// New source code to replace the entire script content
    #[schemars(description = "New script source code")]
    pub source: String,
}

/// Parameters for edit_script_lines tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditScriptLinesParams {
    /// Path to the script instance
    #[schemars(description = "Script path (e.g., 'ServerScriptService/Main')")]
    pub path: String,
    /// Start line to replace (1-indexed)
    #[schemars(description = "Start line to replace (1-indexed)")]
    pub start_line: u32,
    /// End line to replace (1-indexed, inclusive)
    #[schemars(description = "End line to replace (1-indexed, inclusive)")]
    pub end_line: u32,
    /// New content to replace the specified lines
    #[schemars(description = "New content to replace the specified line range")]
    pub new_content: String,
}

/// Parameters for set_property tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetPropertyParams {
    /// Instance path (e.g., "Workspace/SpawnLocation")
    #[schemars(description = "Instance path (e.g., 'Workspace/SpawnLocation')")]
    pub path: String,
    /// Property name (e.g., "Anchored", "Transparency")
    #[schemars(description = "Property name (e.g., 'Anchored', 'Transparency', 'Name')")]
    pub property: String,
    /// Value to set. Strings, numbers, booleans, or objects for Vector3/Color3.
    #[schemars(description = "Value - string, number, boolean, or {\"X\":1,\"Y\":2,\"Z\":3} for Vector3")]
    pub value: serde_json::Value,
}

/// Parameters for mass_set_property tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MassSetPropertyParams {
    /// ClassName to filter by (e.g., "Part", "MeshPart")
    #[schemars(description = "ClassName filter (e.g., 'Part', 'MeshPart')")]
    #[serde(rename = "className")]
    pub class_name: String,
    /// Optional parent path to scope the search
    #[schemars(description = "Optional parent path to scope search")]
    pub parent: Option<String>,
    /// Property name to set
    #[schemars(description = "Property name to set")]
    pub property: String,
    /// Value to set on all matching instances
    #[schemars(description = "Property value to set on all matches")]
    pub value: serde_json::Value,
}

/// Parameters for search_by_property tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchByPropertyParams {
    /// Property name to search by
    #[schemars(description = "Property name to search by")]
    pub property: String,
    /// Value to match
    #[schemars(description = "Property value to match")]
    pub value: serde_json::Value,
    /// Optional ClassName filter
    #[schemars(description = "Optional ClassName filter")]
    #[serde(rename = "className")]
    pub class_name: Option<String>,
    /// Optional parent path to scope the search
    #[schemars(description = "Optional parent path to scope search")]
    pub parent: Option<String>,
    /// Maximum results (default: 50)
    #[schemars(description = "Max results (default: 50)")]
    pub limit: Option<u32>,
}

/// Parameters for create_instance tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateInstanceParams {
    /// ClassName of the instance to create (e.g., "Part", "Script", "Folder")
    #[schemars(description = "ClassName to create (e.g., 'Part', 'Script', 'Folder')")]
    #[serde(rename = "className")]
    pub class_name: String,
    /// Parent path (e.g., "Workspace", "ServerScriptService/MyFolder")
    #[schemars(description = "Parent path (e.g., 'Workspace', 'ServerScriptService/MyFolder')")]
    pub parent: String,
    /// Optional name for the new instance
    #[schemars(description = "Instance name (optional, defaults to ClassName)")]
    pub name: Option<String>,
    /// Optional properties to set as JSON object (e.g., {"Anchored": true, "Transparency": 0.5})
    #[schemars(description = "Initial properties as JSON (e.g., {\"Anchored\": true})")]
    pub properties: Option<serde_json::Value>,
}

/// Parameters for delete_instance tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteInstanceParams {
    /// Instance path to delete (e.g., "Workspace/OldPart")
    #[schemars(description = "Instance path to delete (e.g., 'Workspace/OldPart')")]
    pub path: String,
}

/// Parameters for duplicate_instance tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DuplicateInstanceParams {
    /// Instance path to duplicate (e.g., "Workspace/TemplatePart")
    #[schemars(description = "Instance path to duplicate")]
    pub path: String,
    /// Optional new name for the duplicate
    #[schemars(description = "Name for the duplicate (optional)")]
    pub name: Option<String>,
    /// Optional new parent path (defaults to same parent)
    #[schemars(description = "New parent path (optional, defaults to same parent)")]
    pub parent: Option<String>,
}

/// Parameters for get_selection tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSelectionParams {
    /// If true, include properties of selected instances
    #[schemars(description = "Include properties of selected instances (default: false)")]
    pub include_properties: Option<bool>,
}

/// Parameters for get_class_info tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetClassInfoParams {
    /// Roblox class name (e.g., "Part", "Script", "Model")
    #[schemars(description = "ClassName to get info for (e.g., 'Part', 'Script', 'Model')")]
    #[serde(rename = "className")]
    pub class_name: String,
}

/// Parameters for set_active_place tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetActivePlaceParams {
    /// The Roblox Place ID to set as the active target
    #[schemars(description = "The Roblox Place ID to set as the active target")]
    pub place_id: u64,
}

/// Generate Luau code to find a scoped root from a parent path.
fn luau_scope_snippet(parent: &Option<String>) -> String {
    match parent {
        Some(p) => format!(
            r#"local parts = string.split("{}", "/")
local root = game:GetService(parts[1])
for i = 2, #parts do
    root = root:FindFirstChild(parts[i])
    if not root then return "Error: Parent not found" end
end"#,
            escape_luau_string(p)
        ),
        None => "local root = game".to_string(),
    }
}

fn mcp_error(msg: impl Into<String>) -> McpError {
    McpError {
        code: ErrorCode(-32603),
        message: Cow::from(msg.into()),
        data: None,
    }
}

impl RbxSyncServer {
    /// Resolve which place_id to target: explicit param > session state > None (server picks).
    async fn resolve_place_id(&self, explicit: Option<u64>) -> Option<u64> {
        if let Some(id) = explicit {
            return Some(id);
        }
        if let Some(id) = *self.active_place_id.read().await {
            return Some(id);
        }
        None // Let server pick most-recently-active
    }

    /// Check if a playtest is currently running. Returns an error message if not.
    async fn check_playtest_active(&self) -> Option<String> {
        match self.client.get_playtest_status().await {
            Ok(status) => {
                if let Some(data) = &status.data {
                    let running = data.get("running").and_then(|v| v.as_bool()).unwrap_or(false);
                    if !running {
                        return Some("No active playtest. Start one with start_playtest or run_test first.".to_string());
                    }
                }
                None
            }
            Err(_) => None, // Don't block on status check failure
        }
    }
}

impl Default for RbxSyncServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl RbxSyncServer {
    pub fn new() -> Self {
        Self {
            client: RbxSyncClient::new(44755),
            tool_router: Self::tool_router(),
            active_place_id: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Check server connection and return a user-friendly error if not connected.
    /// Returns Ok(None) if connected, Ok(Some(result)) with error message if not.
    async fn require_connection(&self) -> Result<Option<CallToolResult>, McpError> {
        match self.client.check_health().await {
            Ok(true) => Ok(None),
            Ok(false) => Ok(Some(CallToolResult::success(vec![Content::text(
                "Error: RbxSync server is running but not healthy. Check 'rbxsync serve' output for errors.",
            )]))),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Connection refused") || msg.contains("connect") {
                    Ok(Some(CallToolResult::success(vec![Content::text(
                        "Error: Cannot connect to RbxSync server. Make sure 'rbxsync serve' is running.",
                    )])))
                } else {
                    Ok(Some(CallToolResult::success(vec![Content::text(format!(
                        "Error: RbxSync server connection check failed: {}",
                        msg
                    ))])))
                }
            }
        }
    }

    /// Set the active Roblox place for subsequent commands.
    /// Use to target a specific Studio instance in multi-place projects.
    #[tool(description = "Set the active Roblox place for subsequent commands. Use to target a specific Studio instance in multi-place projects.")]
    async fn set_active_place(
        &self,
        Parameters(params): Parameters<SetActivePlaceParams>,
    ) -> Result<CallToolResult, McpError> {
        *self.active_place_id.write().await = Some(params.place_id);
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Active place set to PlaceId: {}. Subsequent commands will target this place unless overridden.",
            params.place_id
        ))]))
    }

    /// Extract a Roblox game from Studio to git-friendly files on disk.
    #[tool(description = "Extract a Roblox game from Studio to git-friendly files")]
    async fn extract_game(
        &self,
        Parameters(params): Parameters<ExtractParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }

        // Resolve place targeting for multi-place projects
        let resolved_place = self.resolve_place_id(params.place_id).await;
        let session_id = if let Some(pid) = resolved_place {
            self.client
                .resolve_session_for_place(pid)
                .await
                .map_err(|e| mcp_error(e.to_string()))?
        } else {
            None
        };

        // Start extraction (pass session_id for place-aware routing; server ignores until Task 4)
        let mut extract_body = serde_json::json!({
            "project_dir": params.project_dir,
            "include_terrain": params.include_terrain,
        });
        if let Some(services) = &params.services {
            extract_body["services"] = serde_json::json!(services);
        }
        if let Some(sid) = &session_id {
            extract_body["session_id"] = serde_json::json!(sid);
        }

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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }

        // Multi-place sync: iterate each place_id, resolve session, sync individually
        if let Some(place_ids) = &params.place_ids {
            let mut per_place_results = Vec::new();
            for &pid in place_ids {
                let session_id = self
                    .client
                    .resolve_session_for_place(pid)
                    .await
                    .map_err(|e| mcp_error(e.to_string()))?;

                let incremental = self
                    .client
                    .read_incremental(&params.project_dir)
                    .await
                    .map_err(|e| mcp_error(e.to_string()))?;

                let operations = tools::build_sync_operations(incremental.instances);
                if operations.is_empty() {
                    per_place_results.push(serde_json::json!({
                        "place_id": pid,
                        "session_id": session_id,
                        "status": "no_changes",
                    }));
                    continue;
                }

                let result = self
                    .client
                    .sync_batch(&operations, Some(&params.project_dir))
                    .await
                    .map_err(|e| mcp_error(e.to_string()))?;

                let applied = result.data.as_ref().map(|d| d.applied).unwrap_or(result.applied);
                per_place_results.push(serde_json::json!({
                    "place_id": pid,
                    "session_id": session_id,
                    "success": result.success,
                    "applied": applied,
                }));
            }
            let _ = self.client.mark_synced(&params.project_dir).await;
            let summary = serde_json::to_string_pretty(&per_place_results)
                .unwrap_or_else(|_| format!("{:?}", per_place_results));
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Multi-place sync complete:\n{}",
                summary
            ))]));
        }

        // Single-place resolution: explicit param > session state > server default
        let resolved_place = self.resolve_place_id(params.place_id).await;
        let _session_id = if let Some(pid) = resolved_place {
            self.client
                .resolve_session_for_place(pid)
                .await
                .map_err(|e| mcp_error(e.to_string()))?
        } else {
            None
        };

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
        // session_id will be used by server-side routing once Task 4 lands
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

    /// Compare local project files against Studio state.
    /// Shows what instances exist locally but not in Studio (added),
    /// what exists in Studio but not locally (removed), and unchanged count.
    #[tool(description = "Diff local project files vs Studio state - shows added, removed, and unchanged instances")]
    async fn diff(
        &self,
        Parameters(params): Parameters<DiffParams>,
    ) -> Result<CallToolResult, McpError> {
        let diff = self.client
            .get_diff(&params.project_dir)
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        let mut lines = Vec::new();
        lines.push(format!(
            "Diff: {} added, {} removed, {} unchanged",
            diff.added.len(),
            diff.removed.len(),
            diff.unchanged
        ));

        if !diff.added.is_empty() {
            lines.push(String::new());
            lines.push(format!("Added ({}):", diff.added.len()));
            for entry in &diff.added {
                let class = entry.class_name.as_deref().unwrap_or("?");
                lines.push(format!("  + [{}] {}", class, entry.path));
            }
        }

        if !diff.removed.is_empty() {
            lines.push(String::new());
            lines.push(format!("Removed ({}):", diff.removed.len()));
            for entry in &diff.removed {
                let class = entry.class_name.as_deref().unwrap_or("?");
                lines.push(format!("  - [{}] {}", class, entry.path));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(lines.join("\n"))]))
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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }

        // Resolve place targeting for multi-place projects
        let resolved_place = self.resolve_place_id(params.place_id).await;
        let _session_id = if let Some(pid) = resolved_place {
            self.client
                .resolve_session_for_place(pid)
                .await
                .map_err(|e| mcp_error(e.to_string()))?
        } else {
            None
        };
        // session_id will be used by server-side routing once Task 4 lands

        let result = self.client.run_code(&params.code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Run an automated play test in Roblox Studio and capture console output.
    /// Starts a play session, captures all prints/warnings/errors, then stops and returns output.
    /// For interactive bot testing, use background: true to start the test and return immediately,
    /// then use bot_observe/bot_move/bot_action while the test runs.
    /// IMPORTANT: Stop playtest with stop_playtest before making code changes.
    /// Changes won't take effect until you stop the playtest, sync, then run_test again.
    #[tool(description = "Run automated play test in Studio and return console output. For interactive bot testing, use background: true to start test and return immediately. IMPORTANT: Stop playtest with stop_playtest before making code changes.")]
    async fn run_test(
        &self,
        Parameters(params): Parameters<RunTestParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }

        // Resolve place targeting for multi-place projects
        let resolved_place = self.resolve_place_id(params.place_id).await;
        let _session_id = if let Some(pid) = resolved_place {
            self.client
                .resolve_session_for_place(pid)
                .await
                .map_err(|e| mcp_error(e.to_string()))?
        } else {
            None
        };
        // session_id will be used by server-side routing once Task 4 lands

        // Use the new playtest control endpoints for reliable lifecycle management
        let start_result = self.client
            .start_playtest(params.mode.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !start_result.success {
            let error_msg = start_result.error
                .or(start_result.message)
                .unwrap_or_else(|| "Unknown error".to_string());
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to start test: {}", error_msg
            ))]));
        }

        // Background mode: return immediately after starting the playtest
        if params.background.unwrap_or(false) {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "started": true,
                    "mode": params.mode.as_deref().unwrap_or("Play"),
                    "message": "Test started in background. Use bot_observe/bot_move/bot_action to interact. Call stop_playtest when done."
                }).to_string()
            )]));
        }

        // Wait for the specified duration, polling status to detect early termination
        let duration_secs = params.duration.unwrap_or(5);
        let poll_interval = tokio::time::Duration::from_millis(500);
        let max_wait = tokio::time::Duration::from_secs((duration_secs + 2) as u64);
        let start_time = tokio::time::Instant::now();

        loop {
            tokio::time::sleep(poll_interval).await;

            // Check if playtest ended early
            let status = self.client.get_playtest_status().await;
            if let Ok(status) = status {
                if let Some(data) = &status.data {
                    let running = data.get("running").and_then(|v| v.as_bool()).unwrap_or(true);
                    if !running {
                        break;
                    }
                }
            }

            if start_time.elapsed() > max_wait {
                break;
            }
        }

        // Stop the playtest and collect output
        let stop_result = self.client.stop_playtest().await.map_err(|e| mcp_error(e.to_string()))?;

        let elapsed = start_time.elapsed().as_secs_f64();
        let mut output_lines = vec![
            format!("Test completed in {:.1}s", elapsed),
        ];

        if let Some(data) = &stop_result.data {
            let total_messages = data.get("totalMessages").and_then(|v| v.as_i64()).unwrap_or(0);
            output_lines.push(format!("Total messages: {}", total_messages));
            output_lines.push(String::new());

            if let Some(messages) = data.get("output").and_then(|v| v.as_array()) {
                let errors: Vec<_> = messages.iter()
                    .filter(|m| m.get("type").and_then(|v| v.as_str()) == Some("MessageError"))
                    .collect();
                let warnings: Vec<_> = messages.iter()
                    .filter(|m| m.get("type").and_then(|v| v.as_str()) == Some("MessageWarning"))
                    .collect();
                let prints: Vec<_> = messages.iter()
                    .filter(|m| m.get("type").and_then(|v| v.as_str()) == Some("MessageOutput"))
                    .collect();

                if !errors.is_empty() {
                    output_lines.push(format!("=== ERRORS ({}) ===", errors.len()));
                    for msg in &errors {
                        let ts = msg.get("timestamp").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let text = msg.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        output_lines.push(format!("[{:.2}s] {}", ts, text));
                    }
                    output_lines.push(String::new());
                }
                if !warnings.is_empty() {
                    output_lines.push(format!("=== WARNINGS ({}) ===", warnings.len()));
                    for msg in &warnings {
                        let ts = msg.get("timestamp").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let text = msg.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        output_lines.push(format!("[{:.2}s] {}", ts, text));
                    }
                    output_lines.push(String::new());
                }
                if !prints.is_empty() {
                    output_lines.push(format!("=== OUTPUT ({}) ===", prints.len()));
                    for msg in &prints {
                        let ts = msg.get("timestamp").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let text = msg.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        output_lines.push(format!("[{:.2}s] {}", ts, text));
                    }
                }
            }

            if let Some(err) = data.get("error").and_then(|v| v.as_str()) {
                if !err.is_empty() {
                    output_lines.insert(0, format!("Test error: {}", err));
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(output_lines.join("\n"))]))
    }

    /// Stop any running playtest in Roblox Studio.
    /// Call this before making code changes - changes won't take effect until you stop the test,
    /// sync your changes, then run a new test.
    /// Delegates to stop_playtest for consistent lifecycle management.
    #[tool(description = "Stop any running playtest. Call before making code changes.")]
    async fn stop_test(&self) -> Result<CallToolResult, McpError> {
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
        let result = self.client.stop_playtest().await.map_err(|e| mcp_error(e.to_string()))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(
                "Playtest stopped successfully.".to_string()
            )]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to stop playtest: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            ))]))
        }
    }

    // ========================================================================
    // Playtest Control Tools (HTTP-driven playtest lifecycle)
    // ========================================================================

    /// Start a playtest session in Roblox Studio.
    /// Unlike run_test, this starts the playtest without an auto-stop timer.
    /// The playtest runs until explicitly stopped via stop_playtest.
    /// Use this for interactive testing with bot tools.
    #[tool(description = "Start a playtest session in Studio. Runs until stop_playtest is called. Use for interactive bot testing.")]
    async fn start_playtest(
        &self,
        #[allow(unused)] Parameters(params): Parameters<StartPlaytestParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .start_playtest(params.mode.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            let error_msg = result.error
                .or(result.message.clone())
                .or_else(|| result.data.as_ref().and_then(|d| d.get("message").and_then(|v| v.as_str()).map(|s| s.to_string())))
                .unwrap_or_else(|| "Unknown error".to_string());
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to start playtest: {}", error_msg
            ))]));
        }

        let mode = params.mode.as_deref().unwrap_or("Play");
        let message = result.message
            .or_else(|| result.data.as_ref().and_then(|d| d.get("message").and_then(|v| v.as_str()).map(|s| s.to_string())))
            .unwrap_or_else(|| format!("Playtest started (mode: {})", mode));

        Ok(CallToolResult::success(vec![Content::text(format!(
            "{}. Use bot_observe/bot_move/bot_action to interact, then stop_playtest when done.",
            message
        ))]))
    }

    /// Stop the current playtest in Roblox Studio.
    /// Returns captured console output from the playtest session.
    #[tool(description = "Stop the current playtest and return captured console output")]
    async fn stop_playtest(
        &self,
        #[allow(unused)] Parameters(_params): Parameters<StopPlaytestParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .stop_playtest()
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to stop playtest: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            ))]));
        }

        // Format captured output from the test session
        let mut output_lines = vec!["Playtest stopped.".to_string()];

        if let Some(data) = &result.data {
            if let Some(total) = data.get("totalMessages").and_then(|v| v.as_i64()) {
                output_lines.push(format!("Total messages captured: {}", total));
            }
            if let Some(duration) = data.get("duration").and_then(|v| v.as_f64()) {
                output_lines.push(format!("Duration: {:.1}s", duration));
            }
            if let Some(messages) = data.get("output").and_then(|v| v.as_array()) {
                if !messages.is_empty() {
                    output_lines.push(String::new());
                    let errors: Vec<_> = messages.iter()
                        .filter(|m| m.get("type").and_then(|v| v.as_str()) == Some("MessageError"))
                        .collect();
                    let warnings: Vec<_> = messages.iter()
                        .filter(|m| m.get("type").and_then(|v| v.as_str()) == Some("MessageWarning"))
                        .collect();
                    let prints: Vec<_> = messages.iter()
                        .filter(|m| m.get("type").and_then(|v| v.as_str()) == Some("MessageOutput"))
                        .collect();

                    if !errors.is_empty() {
                        output_lines.push(format!("=== ERRORS ({}) ===", errors.len()));
                        for msg in &errors {
                            let ts = msg.get("timestamp").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let text = msg.get("message").and_then(|v| v.as_str()).unwrap_or("");
                            output_lines.push(format!("[{:.2}s] {}", ts, text));
                        }
                        output_lines.push(String::new());
                    }
                    if !warnings.is_empty() {
                        output_lines.push(format!("=== WARNINGS ({}) ===", warnings.len()));
                        for msg in &warnings {
                            let ts = msg.get("timestamp").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let text = msg.get("message").and_then(|v| v.as_str()).unwrap_or("");
                            output_lines.push(format!("[{:.2}s] {}", ts, text));
                        }
                        output_lines.push(String::new());
                    }
                    if !prints.is_empty() {
                        output_lines.push(format!("=== OUTPUT ({}) ===", prints.len()));
                        for msg in &prints {
                            let ts = msg.get("timestamp").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let text = msg.get("message").and_then(|v| v.as_str()).unwrap_or("");
                            output_lines.push(format!("[{:.2}s] {}", ts, text));
                        }
                    }
                }
            }
            if let Some(err) = data.get("error").and_then(|v| v.as_str()) {
                if !err.is_empty() {
                    output_lines.push(format!("\nPlaytest error: {}", err));
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(output_lines.join("\n"))]))
    }

    /// Get the current playtest status.
    /// Returns whether a playtest is running, the mode, and capture state.
    #[tool(description = "Get current playtest status - running state, mode, capture info")]
    async fn playtest_status(
        &self,
        #[allow(unused)] Parameters(_params): Parameters<PlaytestStatusParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.client
            .get_playtest_status()
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        if !result.success {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Failed to get playtest status: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            ))]));
        }

        let mut output_lines = vec![];

        if let Some(data) = &result.data {
            let running = data.get("running").and_then(|v| v.as_bool()).unwrap_or(false);
            let mode = data.get("mode").and_then(|v| v.as_str()).unwrap_or("unknown");
            let capturing = data.get("capturing").and_then(|v| v.as_bool()).unwrap_or(false);
            let total_messages = data.get("totalMessages").and_then(|v| v.as_i64()).unwrap_or(0);

            output_lines.push(format!("Running: {}", running));
            output_lines.push(format!("Mode: {}", mode));
            output_lines.push(format!("Capturing: {}", capturing));
            output_lines.push(format!("Messages captured: {}", total_messages));

            if let Some(err) = data.get("error").and_then(|v| v.as_str()) {
                if !err.is_empty() {
                    output_lines.push(format!("Error: {}", err));
                }
            }
        } else {
            output_lines.push("No status data available.".to_string());
        }

        Ok(CallToolResult::success(vec![Content::text(output_lines.join("\n"))]))
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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
        if let Some(msg) = self.check_playtest_active().await {
            return Ok(CallToolResult::success(vec![Content::text(msg)]));
        }
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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
        if let Some(msg) = self.check_playtest_active().await {
            return Ok(CallToolResult::success(vec![Content::text(msg)]));
        }

        // Validate: at least one of position or objectName must be provided
        if params.position.is_none() && params.object_name.is_none() {
            return Ok(CallToolResult::error(vec![Content::text(
                "Error: Must provide either 'position' ({x, y, z}) or 'objectName' (string)."
            )]));
        }

        // Validate position format if provided
        if let Some(ref pos) = params.position {
            let valid = pos.get("x").and_then(|v| v.as_f64()).is_some()
                && pos.get("y").and_then(|v| v.as_f64()).is_some()
                && pos.get("z").and_then(|v| v.as_f64()).is_some();
            if !valid {
                return Ok(CallToolResult::error(vec![Content::text(
                    "Error: position must be an object with numeric x, y, z fields. Example: {\"x\": 0, \"y\": 5, \"z\": 0}"
                )]));
            }
        }

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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
        if let Some(msg) = self.check_playtest_active().await {
            return Ok(CallToolResult::success(vec![Content::text(msg)]));
        }
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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
        if let Some(msg) = self.check_playtest_active().await {
            return Ok(CallToolResult::success(vec![Content::text(msg)]));
        }
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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
        if let Some(msg) = self.check_playtest_active().await {
            return Ok(CallToolResult::success(vec![Content::text(msg)]));
        }
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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
        if let Some(msg) = self.check_playtest_active().await {
            return Ok(CallToolResult::success(vec![Content::text(msg)]));
        }
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
    // CollectionService Tag Management Tools (Issue #133)
    // ========================================================================

    /// Get all tags on an instance.
    #[tool(description = "List all tags on an instance")]
    async fn get_tags(
        &self,
        Parameters(params): Parameters<GetTagsParams>,
    ) -> Result<CallToolResult, McpError> {
        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);

        let code = format!(
            "{navigate}\n\
            local CollectionService = game:GetService(\"CollectionService\")\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            local tags = CollectionService:GetTags(target)\n\
            if #tags == 0 then return \"No tags on \" .. target:GetFullName() end\n\
            return \"Tags on \" .. target:GetFullName() .. \" (\" .. #tags .. \"):\\n\" .. table.concat(tags, \"\\n\")",
            navigate = navigate,
            path = path_escaped,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Add a tag to an instance.
    #[tool(description = "Add a tag to an instance")]
    async fn add_tag(
        &self,
        Parameters(params): Parameters<AddTagParams>,
    ) -> Result<CallToolResult, McpError> {
        validate_luau_identifier(&params.tag).map_err(|e| {
            mcp_error(format!("Invalid tag name: {}", e))
        })?;

        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);
        let tag_escaped = escape_luau_string(&params.tag);

        let code = format!(
            "{navigate}\n\
            local CollectionService = game:GetService(\"CollectionService\")\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            CollectionService:AddTag(target, \"{tag}\")\n\
            return \"Added tag '{tag}' to \" .. target:GetFullName()",
            navigate = navigate,
            path = path_escaped,
            tag = tag_escaped,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Remove a tag from an instance.
    #[tool(description = "Remove a tag from an instance")]
    async fn remove_tag(
        &self,
        Parameters(params): Parameters<RemoveTagParams>,
    ) -> Result<CallToolResult, McpError> {
        validate_luau_identifier(&params.tag).map_err(|e| {
            mcp_error(format!("Invalid tag name: {}", e))
        })?;

        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);
        let tag_escaped = escape_luau_string(&params.tag);

        let code = format!(
            "{navigate}\n\
            local CollectionService = game:GetService(\"CollectionService\")\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            if not CollectionService:HasTag(target, \"{tag}\") then\n\
                return \"Tag '{tag}' not found on \" .. target:GetFullName()\n\
            end\n\
            CollectionService:RemoveTag(target, \"{tag}\")\n\
            return \"Removed tag '{tag}' from \" .. target:GetFullName()",
            navigate = navigate,
            path = path_escaped,
            tag = tag_escaped,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Find all instances with a specific tag.
    /// Useful for understanding game structure -- e.g., find all "Enemy" or "Collectible" instances.
    #[tool(description = "Find all instances with a specific tag")]
    async fn get_tagged(
        &self,
        Parameters(params): Parameters<GetTaggedParams>,
    ) -> Result<CallToolResult, McpError> {
        validate_luau_identifier(&params.tag).map_err(|e| {
            mcp_error(format!("Invalid tag name: {}", e))
        })?;

        let tag_escaped = escape_luau_string(&params.tag);
        let limit = params.limit.unwrap_or(100).min(500);

        let code = format!(
            "local CollectionService = game:GetService(\"CollectionService\")\n\
            local instances = CollectionService:GetTagged(\"{tag}\")\n\
            if #instances == 0 then return \"No instances found with tag '{tag}'\" end\n\
            local results = {{}}\n\
            local limit = {limit}\n\
            for i, inst in instances do\n\
                if i > limit then break end\n\
                table.insert(results, inst:GetFullName() .. \" [\" .. inst.ClassName .. \"]\")\n\
            end\n\
            local header = \"Found \" .. #instances .. \" instances with tag '{tag}'\"\n\
            if #instances > limit then header = header .. \" (showing first \" .. limit .. \")\" end\n\
            return header .. \":\\n\" .. table.concat(results, \"\\n\")",
            tag = tag_escaped,
            limit = limit,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // ========================================================================
    // Attribute Management Tools (Issue #132)
    // ========================================================================

    /// Get all attributes on an instance.
    /// Returns a JSON object with attribute name-value pairs.
    #[tool(description = "Get all attributes on an instance by path")]
    async fn get_attributes(
        &self,
        Parameters(params): Parameters<GetAttributesParams>,
    ) -> Result<CallToolResult, McpError> {
        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);

        // Luau-side escaping: replace \ with \\ then " with \" in both keys and string values
        let code = format!(
            "{navigate}\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            local attrs = target:GetAttributes()\n\
            local parts = {{}}\n\
            local function escapeStr(s)\n\
                s = string.gsub(s, '\\\\', '\\\\\\\\')\n\
                s = string.gsub(s, '\"', '\\\\\"')\n\
                return s\n\
            end\n\
            for name, value in pairs(attrs) do\n\
                local valStr = tostring(value)\n\
                if type(value) == \"string\" then valStr = '\"' .. escapeStr(value) .. '\"' end\n\
                table.insert(parts, '\"' .. escapeStr(name) .. '\": ' .. valStr)\n\
            end\n\
            if #parts == 0 then return \"No attributes on \" .. target:GetFullName() end\n\
            return \"Attributes on \" .. target:GetFullName() .. \":\\n{{\" .. table.concat(parts, \", \") .. \"}}\"",
            navigate = navigate,
            path = path_escaped,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Set an attribute on an instance.
    /// Creates the attribute if it doesn't exist.
    #[tool(description = "Set an attribute on an instance (creates if new)")]
    async fn set_attribute(
        &self,
        Parameters(params): Parameters<SetAttributeParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate attribute name to prevent Luau injection
        validate_luau_identifier(&params.name).map_err(|e| mcp_error(format!("Invalid attribute name: {}", e)))?;

        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);
        let name_escaped = escape_luau_string(&params.name);
        let value_lua = json_value_to_luau(&params.value);

        let code = format!(
            "{navigate}\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            local ok, err = pcall(function()\n\
                target:SetAttribute(\"{name}\", {value})\n\
            end)\n\
            if not ok then return \"Error: \" .. tostring(err) end\n\
            return \"Set attribute '{name}' = \" .. tostring(target:GetAttribute(\"{name}\")) .. \" on \" .. target:GetFullName()",
            navigate = navigate,
            path = path_escaped,
            name = name_escaped,
            value = value_lua,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Delete an attribute from an instance.
    /// Sets the attribute to nil, which removes it.
    #[tool(description = "Delete an attribute from an instance")]
    async fn delete_attribute(
        &self,
        Parameters(params): Parameters<DeleteAttributeParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate attribute name to prevent Luau injection
        validate_luau_identifier(&params.name).map_err(|e| mcp_error(format!("Invalid attribute name: {}", e)))?;

        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);
        let name_escaped = escape_luau_string(&params.name);

        let code = format!(
            "{navigate}\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            local oldVal = target:GetAttribute(\"{name}\")\n\
            if oldVal == nil then return \"Attribute '{name}' does not exist on \" .. target:GetFullName() end\n\
            target:SetAttribute(\"{name}\", nil)\n\
            return \"Deleted attribute '{name}' (was \" .. tostring(oldVal) .. \") from \" .. target:GetFullName()",
            navigate = navigate,
            path = path_escaped,
            name = name_escaped,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // ========================================================================
    // Script Source Editing Tools (Issue #131)
    // ========================================================================

    /// Read script source code by instance path, with optional line range.
    #[tool(description = "Read script source code by path, with optional line range")]
    async fn get_script_source(
        &self,
        Parameters(params): Parameters<GetScriptSourceParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate line range parameters (Luau arrays are 1-indexed)
        if let Some(s) = params.start_line {
            if s < 1 {
                return Err(mcp_error("start_line must be >= 1 (Luau arrays are 1-indexed)"));
            }
        }
        if let (Some(s), Some(e)) = (params.start_line, params.end_line) {
            if s > e {
                return Err(mcp_error(format!(
                    "start_line ({}) must be <= end_line ({})", s, e
                )));
            }
        }

        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);
        let line_filter = match (params.start_line, params.end_line) {
            (Some(s), Some(e)) => format!(
                "local lines = string.split(source, \"\\n\")\n\
                 local selected = {{}}\n\
                 for i = {}, math.min({}, #lines) do\n\
                     table.insert(selected, i .. \": \" .. lines[i])\n\
                 end\n\
                 return table.concat(selected, \"\\n\")",
                s, e
            ),
            (Some(s), None) => format!(
                "local lines = string.split(source, \"\\n\")\n\
                 local selected = {{}}\n\
                 for i = {}, #lines do\n\
                     table.insert(selected, i .. \": \" .. lines[i])\n\
                 end\n\
                 return table.concat(selected, \"\\n\")",
                s
            ),
            _ => "return source".to_string(),
        };

        let code = format!(
            "{navigate}\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            if not target:IsA(\"LuaSourceContainer\") then return \"Error: Not a script instance\" end\n\
            local source = target.Source\n\
            {line_filter}",
            navigate = navigate,
            path = path_escaped,
            line_filter = line_filter,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Replace the entire source code of a script.
    #[tool(description = "Replace entire script source code")]
    async fn set_script_source(
        &self,
        Parameters(params): Parameters<SetScriptSourceParams>,
    ) -> Result<CallToolResult, McpError> {
        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);
        // Use double-bracket Luau string for source to avoid escaping issues
        let source_escaped = params.source.replace("]]", "] ]");

        let code = format!(
            "{navigate}\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            if not target:IsA(\"LuaSourceContainer\") then return \"Error: Not a script instance\" end\n\
            target.Source = [[{source}]]\n\
            local lineCount = #string.split(target.Source, \"\\n\")\n\
            return \"Set source on \" .. target:GetFullName() .. \" (\" .. lineCount .. \" lines)\"",
            navigate = navigate,
            path = path_escaped,
            source = source_escaped,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Edit specific lines within a script's source code.
    /// Replaces lines from start_line to end_line (inclusive) with new_content.
    #[tool(description = "Edit specific lines in a script (replace a line range with new content)")]
    async fn edit_script_lines(
        &self,
        Parameters(params): Parameters<EditScriptLinesParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate line range parameters (Luau arrays are 1-indexed)
        if params.start_line < 1 {
            return Err(mcp_error("start_line must be >= 1 (Luau arrays are 1-indexed)"));
        }
        if params.start_line > params.end_line {
            return Err(mcp_error(format!(
                "start_line ({}) must be <= end_line ({})", params.start_line, params.end_line
            )));
        }

        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);
        let new_content_escaped = params.new_content.replace("]]", "] ]");

        let code = format!(
            "{navigate}\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            if not target:IsA(\"LuaSourceContainer\") then return \"Error: Not a script instance\" end\n\
            local lines = string.split(target.Source, \"\\n\")\n\
            local startLine = {start_line}\n\
            local endLine = math.min({end_line}, #lines)\n\
            if startLine > #lines then return \"Error: start_line exceeds script length (\" .. #lines .. \" lines)\" end\n\
            local newLines = string.split([[{new_content}]], \"\\n\")\n\
            local result = {{}}\n\
            for i = 1, startLine - 1 do table.insert(result, lines[i]) end\n\
            for _, line in newLines do table.insert(result, line) end\n\
            for i = endLine + 1, #lines do table.insert(result, lines[i]) end\n\
            target.Source = table.concat(result, \"\\n\")\n\
            return \"Edited lines \" .. startLine .. \"-\" .. endLine .. \" in \" .. target:GetFullName() .. \" (now \" .. #result .. \" lines)\"",
            navigate = navigate,
            path = path_escaped,
            start_line = params.start_line,
            end_line = params.end_line,
            new_content = new_content_escaped,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // ========================================================================
    // Property Manipulation Tools (Issue #129)
    // ========================================================================

    /// Set a single property on an instance by path.
    /// Supports booleans, numbers, strings, enums, Vector3, Color3, etc.
    /// For Vector3: use {"X":1,"Y":2,"Z":3}. For Color3: use {"R":255,"G":0,"B":0}.
    #[tool(description = "Set a property on an instance by path (e.g., set Workspace/Part.Anchored to true)")]
    async fn set_property(
        &self,
        Parameters(params): Parameters<SetPropertyParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate property name to prevent Luau injection (used as inst.{property})
        validate_luau_identifier(&params.property).map_err(|e| {
            mcp_error(format!("Invalid property name: {}", e))
        })?;

        let value_lua = json_value_to_luau(&params.value);
        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);
        let code = format!(
            "{navigate}\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            local ok, err = pcall(function()\n\
                target.{property} = {value}\n\
            end)\n\
            if not ok then return \"Error setting property: \" .. tostring(err) end\n\
            return \"Set \" .. target:GetFullName() .. \".{property} = \" .. tostring(target.{property})",
            navigate = navigate,
            path = path_escaped,
            property = params.property,
            value = value_lua,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Set the same property on all instances matching a ClassName filter.
    /// Optionally scoped to a parent path. Returns count of modified instances.
    #[tool(description = "Set a property on all instances of a ClassName (e.g., set all Parts to Anchored=true)")]
    async fn mass_set_property(
        &self,
        Parameters(params): Parameters<MassSetPropertyParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate property name to prevent Luau injection (used as inst.{property})
        validate_luau_identifier(&params.property).map_err(|e| {
            mcp_error(format!("Invalid property name: {}", e))
        })?;
        // Validate class_name even though it's used in a string context (defense in depth)
        validate_luau_identifier(&params.class_name).map_err(|e| {
            mcp_error(format!("Invalid class name: {}", e))
        })?;

        let value_lua = json_value_to_luau(&params.value);
        let scope = luau_scope_snippet(&params.parent);

        let code = format!(
            "local count = 0\n\
            local errors = {{}}\n\
            do\n\
                {scope}\n\
                for _, inst in root:GetDescendants() do\n\
                    if inst:IsA(\"{class_name}\") then\n\
                        local ok, err = pcall(function()\n\
                            inst.{property} = {value}\n\
                        end)\n\
                        if ok then\n\
                            count = count + 1\n\
                        else\n\
                            table.insert(errors, inst:GetFullName() .. \": \" .. tostring(err))\n\
                        end\n\
                    end\n\
                end\n\
            end\n\
            local result = \"Set {property} on \" .. count .. \" {class_name} instances\"\n\
            if #errors > 0 then\n\
                result = result .. \"\\nErrors (\" .. #errors .. \"):\\n\" .. table.concat(errors, \"\\n\")\n\
            end\n\
            return result",
            scope = scope,
            class_name = escape_luau_string(&params.class_name),
            property = params.property,
            value = value_lua,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Find instances where a specific property matches a given value.
    /// Returns paths and classNames of matching instances.
    #[tool(description = "Find instances with a specific property value (e.g., find Parts where Anchored=false)")]
    async fn search_by_property(
        &self,
        Parameters(params): Parameters<SearchByPropertyParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate property name to prevent Luau injection (used as inst.{property})
        validate_luau_identifier(&params.property).map_err(|e| {
            mcp_error(format!("Invalid property name: {}", e))
        })?;

        let limit = params.limit.unwrap_or(50).min(200);
        let class_filter = match &params.class_name {
            Some(c) => {
                validate_luau_identifier(c).map_err(|e| {
                    mcp_error(format!("Invalid class name: {}", e))
                })?;
                format!("inst:IsA(\"{}\")", escape_luau_string(c))
            }
            None => "true".to_string(),
        };
        let scope = luau_scope_snippet(&params.parent);
        let value_lua = json_value_to_luau(&params.value);

        let code = format!(
            "local results = {{}}\n\
            local limit = {limit}\n\
            do\n\
                {scope}\n\
                local targetValue = {value}\n\
                for _, inst in root:GetDescendants() do\n\
                    if {class_filter} then\n\
                        local ok, val = pcall(function() return inst.{property} end)\n\
                        if ok and val == targetValue then\n\
                            table.insert(results, inst:GetFullName() .. \" [\" .. inst.ClassName .. \"]\")\n\
                            if #results >= limit then break end\n\
                        end\n\
                    end\n\
                end\n\
            end\n\
            if #results == 0 then\n\
                return \"No instances found where {property} matches\"\n\
            end\n\
            local header = \"Found \" .. #results .. \" instances\"\n\
            if #results >= limit then header = header .. \" (limit reached)\" end\n\
            return header .. \":\\n\" .. table.concat(results, \"\\n\")",
            limit = limit,
            scope = scope,
            value = value_lua,
            class_filter = class_filter,
            property = params.property,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // ========================================================================
    // Instance Management Tools (Issue #130)
    // ========================================================================

    /// Create a new instance in Studio.
    /// Specify the ClassName, parent path, optional name, and optional initial properties.
    #[tool(description = "Create a new instance in Studio (Part, Script, Folder, etc.)")]
    async fn create_instance(
        &self,
        Parameters(params): Parameters<CreateInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate className to prevent Luau injection
        validate_luau_identifier(&params.class_name).map_err(|e| {
            mcp_error(format!("Invalid class name: {}", e))
        })?;

        let class_escaped = escape_luau_string(&params.class_name);
        let parent_escaped = escape_luau_string(&params.parent);
        let name_line = match &params.name {
            Some(n) => format!("inst.Name = \"{}\"", escape_luau_string(n)),
            None => String::new(),
        };

        // Build property assignment lines with key validation
        let prop_lines = match &params.properties {
            Some(serde_json::Value::Object(map)) => {
                let mut lines = Vec::new();
                for (key, val) in map {
                    // Validate each property key to prevent Luau injection
                    validate_luau_identifier(key).map_err(|e| {
                        mcp_error(format!("Invalid property name '{}': {}", key, e))
                    })?;
                    let luau_val = json_value_to_luau(val);
                    lines.push(format!(
                        "local ok, err = pcall(function() inst.{} = {} end)\n\
                        if not ok then table.insert(errors, \"{}: \" .. tostring(err)) end",
                        key, luau_val, escape_luau_string(key)
                    ));
                }
                lines.join("\n")
            }
            _ => String::new(),
        };

        let navigate = luau_navigate_snippet(&params.parent);

        let code = format!(
            "{navigate}\n\
            if not target then return \"Error: Parent not found at path: {parent}\" end\n\
            local inst = Instance.new(\"{class_name}\")\n\
            {name_line}\n\
            local errors = {{}}\n\
            {prop_lines}\n\
            inst.Parent = target\n\
            local result = \"Created \" .. inst.ClassName .. \" '\" .. inst.Name .. \"' at \" .. inst:GetFullName()\n\
            if #errors > 0 then\n\
                result = result .. \"\\nProperty errors: \" .. table.concat(errors, \"; \")\n\
            end\n\
            return result",
            navigate = navigate,
            parent = parent_escaped,
            class_name = class_escaped,
            name_line = name_line,
            prop_lines = prop_lines,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Delete an instance by path.
    /// The instance and all its descendants will be destroyed.
    #[tool(description = "Delete an instance by path (destroys it and all descendants)")]
    async fn delete_instance(
        &self,
        Parameters(params): Parameters<DeleteInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);

        let code = format!(
            "{navigate}\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            local name = target:GetFullName()\n\
            local className = target.ClassName\n\
            target:Destroy()\n\
            return \"Deleted \" .. className .. \" at \" .. name",
            navigate = navigate,
            path = path_escaped,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Duplicate (clone) an instance by path.
    /// Optionally rename the clone or move it to a different parent.
    #[tool(description = "Duplicate (clone) an instance, optionally with a new name or parent")]
    async fn duplicate_instance(
        &self,
        Parameters(params): Parameters<DuplicateInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let navigate = luau_navigate_snippet(&params.path);
        let path_escaped = escape_luau_string(&params.path);
        let name_line = match &params.name {
            Some(n) => format!("clone.Name = \"{}\"", escape_luau_string(n)),
            None => String::new(),
        };
        let parent_line = match &params.parent {
            Some(p) => {
                let parent_escaped = escape_luau_string(p);
                format!(
                    "local parts2 = string.split(\"{}\", \"/\")\n\
                    local newParent = game:GetService(parts2[1])\n\
                    for i = 2, #parts2 do\n\
                        newParent = newParent and newParent:FindFirstChild(parts2[i])\n\
                    end\n\
                    if not newParent then return \"Error: New parent not found\" end\n\
                    clone.Parent = newParent",
                    parent_escaped
                )
            }
            None => "clone.Parent = target.Parent".to_string(),
        };

        let code = format!(
            "{navigate}\n\
            if not target then return \"Error: Instance not found at path: {path}\" end\n\
            local clone = target:Clone()\n\
            {name_line}\n\
            {parent_line}\n\
            return \"Duplicated to \" .. clone:GetFullName()",
            navigate = navigate,
            path = path_escaped,
            name_line = name_line,
            parent_line = parent_line,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // ========================================================================
    // Selection & Reflection Tools (Issue #134)
    // ========================================================================

    /// Get the currently selected instances in Studio.
    /// Returns paths, classNames, and optionally properties.
    #[tool(description = "Get the currently selected instances in Roblox Studio")]
    async fn get_selection(
        &self,
        Parameters(params): Parameters<GetSelectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let include_props = params.include_properties.unwrap_or(false);
        let props_code = if include_props {
            r#"
        local props = {}
        pcall(function()
            props.Name = inst.Name
            props.ClassName = inst.ClassName
            if inst:IsA("BasePart") then
                props.Position = tostring(inst.Position)
                props.Size = tostring(inst.Size)
                props.Anchored = inst.Anchored
                props.Material = tostring(inst.Material)
            end
            if inst:IsA("LuaSourceContainer") then
                local src = inst.Source
                props.SourceLines = #string.split(src, "\n")
            end
        end)
        local propStr = ""
        for k, v in props do
            propStr = propStr .. "\n    " .. k .. " = " .. tostring(v)
        end
        table.insert(lines, "  " .. inst:GetFullName() .. " [" .. inst.ClassName .. "]" .. propStr)"#
        } else {
            r#"table.insert(lines, "  " .. inst:GetFullName() .. " [" .. inst.ClassName .. "]")"#
        };

        let code = format!(
            r#"local Selection = game:GetService("Selection")
local selected = Selection:Get()
if #selected == 0 then return "No instances selected in Studio" end
local lines = {{"Selected " .. #selected .. " instances:"}}
for _, inst in selected do
    {props_code}
end
return table.concat(lines, "\n")"#,
            props_code = props_code,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Get metadata about a Roblox class including its properties and base class relationships.
    /// Creates a temporary instance to inspect, so only works for classes that can be instantiated.
    #[tool(description = "Get Roblox class info (properties, base classes) by creating a temporary instance")]
    async fn get_class_info(
        &self,
        Parameters(params): Parameters<GetClassInfoParams>,
    ) -> Result<CallToolResult, McpError> {
        let class_escaped = escape_luau_string(&params.class_name);

        let code = format!(
            r#"local ok, inst = pcall(Instance.new, "{class_name}")
if not ok then return "Error: Cannot create instance of class '{class_name}'. It may be abstract or restricted." end

local lines = {{}}
table.insert(lines, "Class: " .. inst.ClassName)

-- Check known base classes via IsA()
local baseClasses = {{"BasePart", "GuiObject", "LuaSourceContainer", "Model", "Light", "Constraint", "UIComponent"}}
local isA = {{}}
for _, base in baseClasses do
    if inst:IsA(base) then table.insert(isA, base) end
end
if #isA > 0 then
    table.insert(lines, "IsA: " .. table.concat(isA, ", "))
else
    table.insert(lines, "IsA: Instance")
end

-- Probe common properties via pcall to see which ones exist on this class
local propsToCheck = {{
    "Name", "Archivable",
    -- BasePart
    "Anchored", "CanCollide", "CanQuery", "CanTouch", "CastShadow",
    "Color", "Material", "Position", "Orientation", "Size",
    "Transparency", "Shape", "TopSurface", "BottomSurface",
    "Massless", "RootPriority",
    -- LuaSourceContainer
    "Source", "Disabled",
    -- GuiObject
    "BackgroundColor3", "BackgroundTransparency", "BorderColor3",
    "BorderSizePixel", "Visible", "ZIndex", "LayoutOrder", "AnchorPoint",
    -- Model
    "PrimaryPart", "WorldPivot",
    -- Light
    "Brightness", "Color", "Enabled", "Shadows",
    -- Misc
    "Value", "MaxDistance", "SoundId", "Playing",
    "Adornee", "Face", "StudsOffset",
    "MaxForce", "MaxTorque",
}}

table.insert(lines, "\nProperties:")
for _, propName in propsToCheck do
    local valOk, val = pcall(function() return inst[propName] end)
    if valOk then
        table.insert(lines, "  " .. propName .. " = " .. tostring(val) .. " (" .. typeof(val) .. ")")
    end
end

inst:Destroy()
return table.concat(lines, "\n")"#,
            class_name = class_escaped,
        );

        let result = self.client.run_code(&code).await.map_err(|e| mcp_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(result)]))
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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
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
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }
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

    /// Verify game state during playtest. Check properties, counts, distances, backpack items, leaderstats.
    /// Supports timeout for 'eventually true' conditions.
    #[tool(description = "Verify game state during playtest. Check properties, counts, distances, backpack items, leaderstats. Supports timeout for 'eventually true' conditions.")]
    async fn verify(
        &self,
        Parameters(params): Parameters<VerifyParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(err) = self.require_connection().await? {
            return Ok(err);
        }

        // Build check_data JSON from params, only including non-None fields
        let mut check_data = serde_json::json!({
            "check": params.check,
        });
        if let Some(ref path) = params.path {
            check_data["path"] = serde_json::json!(path);
        }
        if let Some(ref property) = params.property {
            check_data["property"] = serde_json::json!(property);
        }
        if let Some(ref operator) = params.operator {
            check_data["operator"] = serde_json::json!(operator);
        }
        if let Some(ref expected) = params.expected {
            check_data["expected"] = expected.clone();
        }
        if let Some(ref message) = params.message {
            check_data["message"] = serde_json::json!(message);
        }
        if let Some(timeout) = params.timeout {
            check_data["timeout"] = serde_json::json!(timeout);
        }
        if let Some(ref tag) = params.tag {
            check_data["tag"] = serde_json::json!(tag);
        }
        if let Some(ref class) = params.class {
            check_data["class"] = serde_json::json!(class);
        }
        if let Some(ref parent) = params.parent {
            check_data["parent"] = serde_json::json!(parent);
        }
        if let Some(ref target) = params.target {
            check_data["target"] = serde_json::json!(target);
        }
        if let Some(ref item) = params.item {
            check_data["item"] = serde_json::json!(item);
        }
        if let Some(ref stat) = params.stat {
            check_data["stat"] = serde_json::json!(stat);
        }

        // Resolve session_id via place targeting
        let resolved_place = self.resolve_place_id(None).await;
        let session_id = if let Some(pid) = resolved_place {
            self.client.resolve_session_for_place(pid).await.ok().flatten()
        } else {
            None
        };

        let result = self.client
            .send_verify(check_data, session_id.as_deref())
            .await
            .map_err(|e| mcp_error(e.to_string()))?;

        // Format result as PASS/FAIL
        let success = result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        let data = result.get("data").cloned().unwrap_or(serde_json::json!({}));
        let error = result.get("error").and_then(|v| v.as_str()).map(|s| s.to_string());

        let msg = data.get("message").and_then(|v| v.as_str()).unwrap_or("(no message)");
        let elapsed = data.get("elapsed").and_then(|v| v.as_f64()).unwrap_or(0.0);

        let mut output = if success {
            format!("PASS: {}", msg)
        } else {
            let mut fail_msg = format!("FAIL: {}", msg);
            if let Some(actual) = data.get("actual") {
                fail_msg.push_str(&format!("\n  actual: {}", actual));
            }
            if let Some(expected) = data.get("expected") {
                fail_msg.push_str(&format!("\n  expected: {}", expected));
            }
            if let Some(ref err) = error {
                fail_msg.push_str(&format!("\n  error: {}", err));
            }
            fail_msg
        };

        if elapsed > 0.01 {
            output.push_str(&format!("\n  (took {:.2}s)", elapsed));
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
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
    // Check for --debug flag and enable RBXSYNC_DEBUG if present
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--debug") {
        std::env::set_var("RBXSYNC_DEBUG", "1");
    }

    // Set up logging to stderr (stdio is for MCP protocol)
    // When --debug or RBXSYNC_DEBUG=1 is set, enable debug-level tracing
    let default_filter = if std::env::var("RBXSYNC_DEBUG").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false) {
        "debug"
    } else {
        "info"
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false),
        )
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter))
        )
        .init();

    tracing::info!("Starting RbxSync MCP server...");
    if std::env::var("RBXSYNC_DEBUG").map(|v| v == "1").unwrap_or(false) {
        tracing::info!("Debug mode enabled (RBXSYNC_DEBUG=1)");
    }

    let service = RbxSyncServer::new().serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}
