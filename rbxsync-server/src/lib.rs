//! RbxSync Server
//!
//! HTTP server that communicates with the Roblox Studio plugin
//! for game extraction and synchronization.

pub mod git;
pub mod file_watcher;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{DefaultBodyLimit, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, watch, Mutex, RwLock};
use uuid::Uuid;

/// Load project config from rbxsync.json
fn load_project_config(project_dir: &str) -> Option<serde_json::Value> {
    let config_path = PathBuf::from(project_dir).join("rbxsync.json");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return Some(config);
            }
        }
    }
    None
}

/// Apply tree mapping to convert DataModel path to filesystem path
fn apply_tree_mapping(datamodel_path: &str, tree_mapping: &HashMap<String, String>) -> String {
    // Try to find longest matching prefix
    let mut best_match: Option<(&str, &str)> = None;
    let mut best_len = 0;

    for (dm_prefix, fs_prefix) in tree_mapping {
        if datamodel_path == dm_prefix || datamodel_path.starts_with(&format!("{}/", dm_prefix)) {
            if dm_prefix.len() > best_len {
                best_match = Some((dm_prefix.as_str(), fs_prefix.as_str()));
                best_len = dm_prefix.len();
            }
        }
    }

    if let Some((dm_prefix, fs_prefix)) = best_match {
        if datamodel_path == dm_prefix {
            fs_prefix.to_string()
        } else {
            let suffix = &datamodel_path[dm_prefix.len() + 1..]; // Skip the '/'
            format!("{}/{}", fs_prefix, suffix)
        }
    } else {
        datamodel_path.to_string()
    }
}

/// Apply reverse tree mapping to convert filesystem path to DataModel path
fn apply_reverse_tree_mapping(fs_path: &str, tree_mapping: &HashMap<String, String>) -> String {
    // Try to find longest matching prefix (reverse lookup)
    let mut best_match: Option<(&str, &str)> = None;
    let mut best_len = 0;

    for (dm_prefix, fs_prefix) in tree_mapping {
        if fs_path == fs_prefix || fs_path.starts_with(&format!("{}/", fs_prefix)) {
            if fs_prefix.len() > best_len {
                best_match = Some((dm_prefix.as_str(), fs_prefix.as_str()));
                best_len = fs_prefix.len();
            }
        }
    }

    if let Some((dm_prefix, fs_prefix)) = best_match {
        if fs_path == fs_prefix {
            dm_prefix.to_string()
        } else {
            let suffix = &fs_path[fs_prefix.len() + 1..]; // Skip the '/'
            format!("{}/{}", dm_prefix, suffix)
        }
    } else {
        fs_path.to_string()
    }
}

/// Extract tree_mapping from config JSON
fn get_tree_mapping(config: &Option<serde_json::Value>) -> HashMap<String, String> {
    config
        .as_ref()
        .and_then(|c| c.get("treeMapping"))
        .and_then(|m| m.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 44755,
            host: "127.0.0.1".to_string(),
        }
    }
}

/// VS Code workspace registration
#[derive(Debug, Clone, Serialize)]
pub struct VsCodeWorkspace {
    pub workspace_dir: String,
    #[serde(skip)]
    pub last_heartbeat: Option<Instant>,
}

/// Console message from Studio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleMessage {
    pub timestamp: String,
    pub message_type: String,  // "info", "warn", "error"
    pub message: String,
    pub source: Option<String>,  // e.g., "sync", "extract", "plugin"
}

/// Max console messages to keep in buffer
const CONSOLE_BUFFER_SIZE: usize = 1000;

/// Shared application state
pub struct AppState {
    /// Queue of pending requests to send to the plugin (legacy/fallback)
    pub request_queue: Mutex<VecDeque<PluginRequest>>,

    /// Per-project request queues for multi-workspace support
    pub project_queues: RwLock<HashMap<String, VecDeque<PluginRequest>>>,

    /// Registry of connected Studio places (session_id → PlaceInfo)
    pub place_registry: RwLock<HashMap<String, PlaceInfo>>,

    /// Registry of connected VS Code workspaces
    pub vscode_workspaces: RwLock<HashMap<String, VsCodeWorkspace>>,

    /// Counter for generating unique session IDs
    pub session_counter: std::sync::atomic::AtomicU64,

    /// Map of request ID to response channel
    pub response_channels: RwLock<HashMap<Uuid, mpsc::UnboundedSender<PluginResponse>>>,

    /// Trigger to wake up long-polling requests
    pub trigger: watch::Sender<()>,

    /// Receiver for trigger notifications
    pub trigger_rx: watch::Receiver<()>,

    /// Active extraction session
    pub extraction_session: RwLock<Option<ExtractionSession>>,

    /// Flag to pause live sync during extraction (avoids syncing files that were just extracted)
    pub live_sync_paused: std::sync::atomic::AtomicBool,

    /// File watcher state for live sync
    pub file_watcher_state: Arc<RwLock<file_watcher::FileWatcherState>>,

    /// Channel to receive file changes
    pub file_change_rx: Mutex<mpsc::UnboundedReceiver<file_watcher::FileChange>>,

    /// Track which VS Code workspaces we've logged (to prevent spam)
    pub logged_vscode_workspaces: RwLock<HashSet<String>>,

    /// Track which Studio places we've logged (to prevent spam)
    pub logged_studio_places: RwLock<HashSet<String>>,

    /// Console message buffer (ring buffer of recent messages)
    pub console_buffer: RwLock<VecDeque<ConsoleMessage>>,

    /// Broadcast channel for real-time console streaming
    pub console_tx: broadcast::Sender<ConsoleMessage>,

    /// Sync state per project (project_dir -> last_sync_time)
    pub sync_state: RwLock<HashMap<String, std::time::SystemTime>>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let (trigger, trigger_rx) = watch::channel(());
        let (file_change_tx, file_change_rx) = mpsc::unbounded_channel();
        let (console_tx, _) = broadcast::channel(100);  // Buffer 100 messages for slow subscribers
        Arc::new(Self {
            request_queue: Mutex::new(VecDeque::new()),
            project_queues: RwLock::new(HashMap::new()),
            place_registry: RwLock::new(HashMap::new()),
            vscode_workspaces: RwLock::new(HashMap::new()),
            session_counter: std::sync::atomic::AtomicU64::new(1),
            response_channels: RwLock::new(HashMap::new()),
            trigger,
            trigger_rx,
            extraction_session: RwLock::new(None),
            live_sync_paused: std::sync::atomic::AtomicBool::new(false),
            file_watcher_state: Arc::new(RwLock::new(file_watcher::FileWatcherState::new(file_change_tx))),
            file_change_rx: Mutex::new(file_change_rx),
            logged_vscode_workspaces: RwLock::new(HashSet::new()),
            logged_studio_places: RwLock::new(HashSet::new()),
            console_buffer: RwLock::new(VecDeque::with_capacity(CONSOLE_BUFFER_SIZE)),
            console_tx,
            sync_state: RwLock::new(HashMap::new()),
        })
    }
}

/// Request to send to the Studio plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequest {
    pub id: Uuid,
    pub command: String,
    pub payload: serde_json::Value,
}

/// Response from the Studio plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    pub id: Uuid,
    pub success: bool,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Active extraction session state
#[derive(Debug)]
pub struct ExtractionSession {
    pub id: String,
    pub chunks_received: usize,
    pub total_chunks: Option<usize>,
    pub data: Vec<serde_json::Value>,
}

/// Connected Studio place information
#[derive(Debug, Clone, Serialize)]
pub struct PlaceInfo {
    pub place_id: u64,
    pub place_name: String,
    pub project_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,  // Unique session ID for this Studio instance
    #[serde(skip)]
    pub last_heartbeat: Option<Instant>,
}

/// Create the main router
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // RbxSync plugin communication endpoints (separate from roblox-mcp)
        .route("/rbxsync/request", get(handle_request_poll))
        .route("/rbxsync/response", post(handle_response))
        .route("/rbxsync/register", post(handle_register))
        .route("/rbxsync/unregister", post(handle_unregister))
        .route("/rbxsync/register-vscode", post(handle_register_vscode))
        .route("/rbxsync/update-project-path", post(handle_update_project_path))
        .route("/rbxsync/link-studio", post(handle_link_studio))
        .route("/rbxsync/unlink-studio", post(handle_unlink_studio))
        .route("/rbxsync/places", get(handle_list_places))
        .route("/rbxsync/workspaces", get(handle_list_workspaces))
        .route("/rbxsync/server-info", get(handle_server_info))
        // New extraction endpoints
        .route("/extract/start", post(handle_extract_start))
        .route("/extract/chunk", post(handle_extract_chunk))
        .route("/extract/status", get(handle_extract_status))
        .route("/extract/export", post(handle_extract_export))
        .route("/extract/finalize", post(handle_extract_finalize))
        .route("/extract/terrain", post(handle_extract_terrain))
        // Sync endpoints
        .route("/sync/command", post(handle_sync_command))
        .route("/sync/batch", post(handle_sync_batch))
        .route("/sync/read-tree", post(handle_sync_read_tree))
        .route("/sync/read-terrain", post(handle_sync_read_terrain))
        .route("/sync/from-studio", post(handle_sync_from_studio))
        .route("/sync/pending-changes", post(handle_sync_pending_changes))
        .route("/sync/incremental", post(handle_sync_incremental))
        // Diff endpoints
        .route("/studio/paths", post(handle_studio_paths))
        .route("/diff", post(handle_diff))
        // Git endpoints
        .route("/git/status", post(handle_git_status))
        .route("/git/log", post(handle_git_log))
        .route("/git/commit", post(handle_git_commit))
        .route("/git/init", post(handle_git_init))
        // Test runner endpoints (for AI-powered development workflows)
        .route("/test/start", post(handle_test_start))
        .route("/test/status", get(handle_test_status))
        .route("/test/stop", post(handle_test_stop))
        // Console output streaming (for E2E testing mode)
        .route("/console/push", post(handle_console_push))
        .route("/console/subscribe", get(handle_console_subscribe))
        .route("/console/history", get(handle_console_history))
        // Run arbitrary Luau code (for MCP)
        .route("/run", post(handle_run_code))
        // Health check
        .route("/health", get(handle_health))
        // Shutdown endpoint
        .route("/shutdown", post(handle_shutdown))
        .with_state(state)
        // Allow large body sizes for extraction chunks (10MB limit)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
}

/// Health check endpoint
async fn handle_health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Shutdown endpoint - gracefully stops the server
async fn handle_shutdown() -> impl IntoResponse {
    tracing::info!("Shutdown requested via API");
    // Spawn a task to exit after response is sent
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        std::process::exit(0);
    });
    Json(serde_json::json!({
        "status": "shutting_down"
    }))
}

/// Register request from Studio plugin
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub place_id: u64,
    pub place_name: String,
    pub project_dir: String,
    #[serde(default)]
    pub session_id: Option<String>,  // Unique session ID for this Studio instance
}

/// Handle Studio plugin registration
async fn handle_register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    let mut registry = state.place_registry.write().await;

    // Use session_id as unique key if provided (handles multiple unpublished places with PlaceId=0)
    // Fall back to place_id for backwards compatibility with older plugins
    let key = req.session_id.clone().unwrap_or_else(|| req.place_id.to_string());

    // Register/update this place (replaces any existing entry for this session)
    registry.insert(key.clone(), PlaceInfo {
        place_id: req.place_id,
        place_name: req.place_name.clone(),
        project_dir: req.project_dir.clone(),
        session_id: req.session_id.clone(),
        last_heartbeat: Some(Instant::now()),
    });
    drop(registry); // Release lock before acquiring another

    // Create project queue if it doesn't exist
    {
        let mut queues = state.project_queues.write().await;
        queues.entry(req.project_dir.clone()).or_insert_with(VecDeque::new);
    }

    // Only log once per session to prevent spam
    let mut logged = state.logged_studio_places.write().await;
    if !logged.contains(&key) {
        logged.insert(key.clone());
        let session_id = state.session_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tracing::info!(
            "Studio registered: {} (PlaceId: {}, Session: {}) -> {}",
            req.place_name,
            req.place_id,
            session_id,
            req.project_dir
        );

        // Check for path mismatch with VS Code workspaces
        let workspaces = state.vscode_workspaces.read().await;
        if !workspaces.is_empty() {
            let studio_dir = req.project_dir.as_str();
            let vscode_dirs: Vec<&str> = workspaces.keys().map(|s| s.as_str()).collect();

            // Check if Studio project matches or is parent/child of any VS Code workspace
            let has_match = vscode_dirs.iter().any(|vscode_dir| {
                *vscode_dir == studio_dir
                    || studio_dir.starts_with(*vscode_dir)
                    || vscode_dir.starts_with(studio_dir)
            });

            if !has_match {
                tracing::warn!(
                    "⚠️  PATH MISMATCH: Studio project is at '{}' but VS Code is open at '{}'",
                    studio_dir,
                    vscode_dirs.join("', '")
                );
                tracing::warn!(
                    "   Extracted files will go to '{}', not your VS Code workspace!",
                    studio_dir
                );
                tracing::warn!(
                    "   To fix: Open VS Code in the Studio project directory."
                );
            }
        }
    }

    Json(serde_json::json!({
        "success": true,
        "message": "Registered successfully"
    }))
}

/// Unregister a Studio place (called when Studio closes)
async fn handle_unregister(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    // Use session_id as unique key if provided (matches register)
    let key = req.session_id.clone().unwrap_or_else(|| req.place_id.to_string());

    let mut registry = state.place_registry.write().await;
    let removed = registry.remove(&key).is_some();

    if removed {
        tracing::info!(
            "Studio unregistered: {} (ID: {}, Session: {:?}) at {}",
            req.place_name,
            req.place_id,
            req.session_id,
            req.project_dir
        );
    }

    Json(serde_json::json!({
        "success": true,
        "removed": removed
    }))
}

/// Clean up stale registrations (no heartbeat in 30 seconds)
/// Must be longer than long-polling timeout (15s) to avoid premature cleanup
async fn cleanup_stale_registrations(state: &Arc<AppState>) {
    let mut registry = state.place_registry.write().await;
    let now = Instant::now();
    let stale_threshold = std::time::Duration::from_secs(30);

    let stale_keys: Vec<String> = registry
        .iter()
        .filter(|(_, info)| {
            info.last_heartbeat
                .map(|t| now.duration_since(t) > stale_threshold)
                .unwrap_or(true)
        })
        .map(|(k, _)| k.clone())
        .collect();

    for key in &stale_keys {
        if let Some(info) = registry.remove(key) {
            tracing::info!("Removed stale registration: {} ({})", info.place_name, key);
        }
    }
}

/// List connected Studio places
async fn handle_list_places(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Clean up stale registrations first
    cleanup_stale_registrations(&state).await;

    let registry = state.place_registry.read().await;
    let places: Vec<&PlaceInfo> = registry.values().collect();

    Json(serde_json::json!({
        "places": places
    }))
}

/// VS Code workspace registration request
#[derive(Debug, Deserialize)]
pub struct RegisterVsCodeRequest {
    pub workspace_dir: String,
}

/// Handle VS Code workspace registration
async fn handle_register_vscode(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterVsCodeRequest>,
) -> impl IntoResponse {
    if req.workspace_dir.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "error": "Empty workspace directory"
        }));
    }

    // Update heartbeat timestamp
    let mut workspaces = state.vscode_workspaces.write().await;
    let is_new = !workspaces.contains_key(&req.workspace_dir);
    workspaces.insert(req.workspace_dir.clone(), VsCodeWorkspace {
        workspace_dir: req.workspace_dir.clone(),
        last_heartbeat: Some(Instant::now()),
    });
    drop(workspaces); // Release lock before acquiring another

    // Only log and start file watcher if this is a new workspace this session
    // Use a separate set to prevent spam from heartbeat registrations
    let mut logged = state.logged_vscode_workspaces.write().await;
    let should_log = !logged.contains(&req.workspace_dir);
    if should_log {
        logged.insert(req.workspace_dir.clone());
        drop(logged); // Release lock

        tracing::info!("VS Code workspace registered: {}", req.workspace_dir);

        // Check for path mismatch with Studio registrations
        let registry = state.place_registry.read().await;
        if !registry.is_empty() {
            let studio_dirs: Vec<&str> = registry.values().map(|p| p.project_dir.as_str()).collect();
            let vscode_dir = req.workspace_dir.as_str();

            // Check if VS Code workspace matches or is parent/child of any Studio project
            let has_match = studio_dirs.iter().any(|studio_dir| {
                vscode_dir == *studio_dir
                    || studio_dir.starts_with(vscode_dir)
                    || vscode_dir.starts_with(*studio_dir)
            });

            if !has_match {
                tracing::warn!(
                    "⚠️  PATH MISMATCH: VS Code is open at '{}' but Studio project is at '{}'",
                    vscode_dir,
                    studio_dirs.join("', '")
                );
                tracing::warn!(
                    "   Extracted files will go to the Studio project path, not your VS Code workspace!"
                );
                tracing::warn!(
                    "   To fix: Open VS Code in the Studio project directory, or run 'rbxsync serve' from there."
                );

                // Return early with mismatch warning
                return Json(serde_json::json!({
                    "success": true,
                    "message": "Workspace registered",
                    "path_mismatch": {
                        "vscode_path": vscode_dir,
                        "studio_paths": studio_dirs,
                        "warning": format!(
                            "VS Code is open at '{}' but Studio project is at '{}'. Extracted files will go to the Studio path, not your VS Code workspace.",
                            vscode_dir,
                            studio_dirs.join("', '")
                        )
                    }
                }));
            }
        }

        // Start file watcher for new workspaces
        if is_new {
            let watcher_state = state.file_watcher_state.clone();
            let dir = req.workspace_dir.clone();
            tokio::spawn(async move {
                if let Err(e) = file_watcher::start_file_watcher(dir, watcher_state).await {
                    tracing::error!("Failed to start file watcher: {}", e);
                }
            });
        }
    }

    Json(serde_json::json!({
        "success": true,
        "message": "Workspace registered"
    }))
}

/// Request to update Studio project path
#[derive(Debug, Deserialize)]
pub struct UpdateProjectPathRequest {
    pub project_dir: String,
}

/// Handle request to update the project path for all connected Studio places
/// This is called when VS Code wants to fix a path mismatch
async fn handle_update_project_path(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateProjectPathRequest>,
) -> impl IntoResponse {
    if req.project_dir.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "error": "Empty project directory"
        }));
    }

    let mut registry = state.place_registry.write().await;
    let mut updated_count = 0;

    // Update all registered places to use the new project directory
    for (_key, place_info) in registry.iter_mut() {
        let old_path = place_info.project_dir.clone();
        place_info.project_dir = req.project_dir.clone();
        updated_count += 1;
        tracing::info!(
            "Updated Studio project path: '{}' -> '{}'",
            old_path,
            req.project_dir
        );
    }

    if updated_count == 0 {
        return Json(serde_json::json!({
            "success": false,
            "error": "No Studio instances connected to update"
        }));
    }

    // Also update project queues to use the new path
    drop(registry);
    {
        let mut queues = state.project_queues.write().await;
        // Move commands from old paths to new path
        let old_keys: Vec<String> = queues.keys().cloned().collect();
        for old_key in old_keys {
            if old_key != req.project_dir {
                if let Some(commands) = queues.remove(&old_key) {
                    queues.entry(req.project_dir.clone())
                        .or_insert_with(VecDeque::new)
                        .extend(commands);
                }
            }
        }
    }

    Json(serde_json::json!({
        "success": true,
        "message": format!("Updated {} Studio instance(s) to use path: {}", updated_count, req.project_dir),
        "updated_count": updated_count
    }))
}

/// Request to link a specific Studio to a workspace
#[derive(Debug, Deserialize)]
pub struct LinkStudioRequest {
    pub place_id: i64,
    pub new_project_dir: String,
}

/// Handle request to link a specific Studio to a workspace
/// This updates the project_dir for a single place
async fn handle_link_studio(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LinkStudioRequest>,
) -> impl IntoResponse {
    let mut registry = state.place_registry.write().await;

    // Find the entry with matching place_id (key is now session_id, not place_id)
    let target_key = registry.iter()
        .find(|(_, place)| place.place_id as i64 == req.place_id)
        .map(|(key, _)| key.clone());

    if let Some(key) = target_key {
        // Auto-unlink any other studios linked to the same workspace
        // This ensures only one studio is linked to each workspace at a time
        let mut unlinked_studios: Vec<String> = Vec::new();
        for (other_key, other_place) in registry.iter_mut() {
            if other_place.project_dir == req.new_project_dir && other_key != &key {
                let old_name = other_place.place_name.clone();
                other_place.project_dir = String::new();
                unlinked_studios.push(old_name);
                tracing::info!(
                    "Auto-unlinked '{}' from {} (new studio linking)",
                    other_place.place_name,
                    req.new_project_dir
                );
            }
        }

        if let Some(place_info) = registry.get_mut(&key) {
            let old_path = place_info.project_dir.clone();
            place_info.project_dir = req.new_project_dir.clone();
            place_info.last_heartbeat = Some(std::time::Instant::now());

            let place_name = place_info.place_name.clone();

            tracing::info!(
                "Linked Studio {} '{}' to workspace: '{}' (was: '{}')",
                req.place_id,
                place_name,
                req.new_project_dir,
                old_path
            );

            return Json(serde_json::json!({
                "success": true,
                "message": format!("Linked {} to {}", place_name, req.new_project_dir),
                "place_name": place_name,
                "auto_unlinked": unlinked_studios
            }));
        }
    }

    Json(serde_json::json!({
        "success": false,
        "error": format!("No Studio found with place_id {}", req.place_id)
    }))
}

/// Request to unlink a Studio from a workspace
#[derive(Debug, Deserialize)]
pub struct UnlinkStudioRequest {
    pub place_id: u64,
}

/// Handle request to unlink a Studio from a workspace
/// This clears the project_dir for a place, effectively unlinking it
async fn handle_unlink_studio(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UnlinkStudioRequest>,
) -> impl IntoResponse {
    let mut registry = state.place_registry.write().await;

    // Find the entry with matching place_id (key is now session_id, not place_id)
    let target_key = registry.iter()
        .find(|(_, place)| place.place_id == req.place_id)
        .map(|(key, _)| key.clone());

    if let Some(key) = target_key {
        if let Some(place_info) = registry.get_mut(&key) {
            let old_path = place_info.project_dir.clone();
            let place_name = place_info.place_name.clone();

            // Clear the project_dir to unlink
            place_info.project_dir = String::new();
            place_info.last_heartbeat = Some(std::time::Instant::now());

            tracing::info!(
                "Unlinked Studio {} '{}' from workspace: '{}'",
                req.place_id,
                place_name,
                old_path
            );

            return Json(serde_json::json!({
                "success": true,
                "message": format!("Unlinked {} from workspace", place_name),
                "place_name": place_name
            }));
        }
    }

    Json(serde_json::json!({
        "success": false,
        "error": format!("No Studio found with place_id {}", req.place_id)
    }))
}

/// Clean up stale VS Code workspace registrations (no heartbeat in 30 seconds)
async fn cleanup_stale_vscode_workspaces(state: &Arc<AppState>) {
    let mut workspaces = state.vscode_workspaces.write().await;
    let now = Instant::now();
    let stale_threshold = std::time::Duration::from_secs(30);

    let stale_keys: Vec<String> = workspaces
        .iter()
        .filter(|(_, ws)| {
            ws.last_heartbeat
                .map(|t| now.duration_since(t) > stale_threshold)
                .unwrap_or(true)
        })
        .map(|(k, _)| k.clone())
        .collect();

    for key in &stale_keys {
        workspaces.remove(key);
        tracing::info!("Removed stale VS Code workspace: {}", key);
    }
}

/// List registered VS Code workspace directories
async fn handle_list_workspaces(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Clean up stale workspaces first
    cleanup_stale_vscode_workspaces(&state).await;

    let workspaces = state.vscode_workspaces.read().await;
    let mut workspace_dirs: Vec<String> = workspaces
        .values()
        .map(|ws| ws.workspace_dir.clone())
        .collect();
    workspace_dirs.sort();

    Json(serde_json::json!({
        "workspaces": workspace_dirs
    }))
}

/// Handle server info request - provides CWD and VS Code workspaces for auto-populating project path
async fn handle_server_info(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    // Get VS Code workspaces - prefer these for auto-populating project path
    let workspaces = state.vscode_workspaces.read().await;
    let vscode_workspaces: Vec<String> = workspaces
        .values()
        .map(|ws| ws.workspace_dir.clone())
        .collect();

    Json(serde_json::json!({
        "cwd": cwd,
        "version": env!("CARGO_PKG_VERSION"),
        "vscode_workspaces": vscode_workspaces,
    }))
}

/// Query params for request polling
#[derive(Debug, Deserialize)]
pub struct RequestPollQuery {
    #[serde(rename = "projectDir")]
    pub project_dir: Option<String>,
}

/// Long-polling endpoint for plugin to receive requests
async fn handle_request_poll(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RequestPollQuery>,
) -> impl IntoResponse {
    // Helper to check queues
    async fn try_pop_request(
        state: &Arc<AppState>,
        project_dir: &Option<String>,
    ) -> Option<PluginRequest> {
        // First try project-specific queue if projectDir provided
        if let Some(ref dir) = project_dir {
            let mut queues = state.project_queues.write().await;
            if let Some(queue) = queues.get_mut(dir) {
                if let Some(request) = queue.pop_front() {
                    return Some(request);
                }
            }
        }

        // Fall back to global queue (legacy support)
        let mut queue = state.request_queue.lock().await;
        queue.pop_front()
    }

    // First check if there's already a request
    if let Some(request) = try_pop_request(&state, &params.project_dir).await {
        return (StatusCode::OK, Json(serde_json::to_value(&request).unwrap()));
    }

    // Update heartbeat for all places matching this projectDir
    if let Some(ref dir) = params.project_dir {
        let mut registry = state.place_registry.write().await;
        for place in registry.values_mut() {
            if place.project_dir == *dir {
                place.last_heartbeat = Some(Instant::now());
            }
        }
    }

    // Wait for a request or timeout after 15 seconds
    let timeout = tokio::time::Duration::from_secs(15);
    let mut trigger_rx = state.trigger_rx.clone();

    tokio::select! {
        _ = tokio::time::sleep(timeout) => {
            // Timeout - return empty response
            (StatusCode::NO_CONTENT, Json(serde_json::json!(null)))
        }
        _ = trigger_rx.changed() => {
            // Check if there's a request
            if let Some(request) = try_pop_request(&state, &params.project_dir).await {
                (StatusCode::OK, Json(serde_json::to_value(&request).unwrap()))
            } else {
                (StatusCode::NO_CONTENT, Json(serde_json::json!(null)))
            }
        }
    }
}

/// Handle response from plugin
async fn handle_response(
    State(state): State<Arc<AppState>>,
    Json(response): Json<PluginResponse>,
) -> impl IntoResponse {
    tracing::info!("Received response for request {}: success={}", response.id, response.success);
    let channels = state.response_channels.read().await;
    if let Some(sender) = channels.get(&response.id) {
        tracing::info!("Found channel for request {}, sending response", response.id);
        let _ = sender.send(response);
    } else {
        tracing::warn!("No channel found for request {} - response dropped", response.id);
    }
    Json(serde_json::json!({"ok": true}))
}

/// Start extraction request
#[derive(Debug, Deserialize)]
pub struct ExtractStartRequest {
    /// Project directory to extract to
    pub project_dir: Option<String>,
    /// Services to extract
    pub services: Option<Vec<String>>,
    /// Include terrain
    pub include_terrain: Option<bool>,
    /// Include binary assets
    pub include_assets: Option<bool>,
}

async fn handle_extract_start(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExtractStartRequest>,
) -> impl IntoResponse {
    tracing::info!("Extract request: include_terrain={:?}", req.include_terrain);
    let session_uuid = Uuid::new_v4();
    let session_id = session_uuid.to_string();

    // Create extraction session
    {
        let mut session = state.extraction_session.write().await;
        *session = Some(ExtractionSession {
            id: session_id.clone(),
            chunks_received: 0,
            total_chunks: None,
            data: Vec::new(),
        });
    }

    // Pause live sync during extraction to avoid syncing back files we just extracted
    state.live_sync_paused.store(true, std::sync::atomic::Ordering::Relaxed);
    tracing::info!("Live sync paused for extraction");

    // Queue request to plugin
    let plugin_request = PluginRequest {
        id: session_uuid,
        command: "extract:start".to_string(),
        payload: serde_json::json!({
            "project_dir": req.project_dir,
            "services": req.services.unwrap_or_default(),
            "extractTerrain": req.include_terrain.unwrap_or(false),
            "includeAssets": req.include_assets.unwrap_or(true),
        }),
    };

    {
        let mut queue = state.request_queue.lock().await;
        queue.push_back(plugin_request);
    }
    let _ = state.trigger.send(());

    Json(serde_json::json!({
        "sessionId": session_id,
        "status": "started"
    }))
}

/// Handle extraction chunk from plugin
#[derive(Debug, Deserialize)]
pub struct ExtractChunkRequest {
    pub session_id: String,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub data: serde_json::Value,
    pub project_dir: Option<String>,
}

async fn handle_extract_chunk(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExtractChunkRequest>,
) -> impl IntoResponse {
    let mut session_guard = state.extraction_session.write().await;

    // Determine output directory: use project_dir/src if provided, otherwise fallback
    let output_dir = if let Some(ref project_dir) = req.project_dir {
        if !project_dir.is_empty() {
            format!("{}/src", project_dir)
        } else {
            format!(".rbxsync/extract_{}", &req.session_id)
        }
    } else {
        format!(".rbxsync/extract_{}", &req.session_id)
    };

    // Auto-create session if plugin started extraction directly
    if session_guard.is_none() {
        tracing::info!("Auto-created extraction session: {} -> {}", &req.session_id, &output_dir);

        // Create output directory for this session
        let _ = std::fs::create_dir_all(&output_dir);

        *session_guard = Some(ExtractionSession {
            id: req.session_id.clone(),
            chunks_received: 0,
            total_chunks: None,
            data: Vec::new(),
        });
    }

    if let Some(ref mut session) = *session_guard {
        // Accept chunks from any session (plugin may have restarted)
        if session.id != req.session_id {
            tracing::info!("Session ID changed from {} to {}, resetting -> {}", session.id, &req.session_id, &output_dir);
            session.id = req.session_id.clone();
            session.chunks_received = 0;
            session.data.clear();

            // Create new output directory
            let _ = std::fs::create_dir_all(&output_dir);
        }

        session.total_chunks = Some(req.total_chunks);
        session.chunks_received += 1;

        // Save chunk to disk immediately
        let chunk_path = format!("{}/chunk_{:06}.json", output_dir, session.chunks_received);
        if let Err(e) = std::fs::write(&chunk_path, serde_json::to_string(&req.data).unwrap_or_default()) {
            tracing::warn!("Failed to save chunk to disk: {}", e);
        }

        // Also keep in memory for quick access
        session.data.push(req.data);

        tracing::info!("Received chunk {}/{}", session.chunks_received, req.total_chunks);

        (
            StatusCode::OK,
            Json(serde_json::json!({
                "received": session.chunks_received,
                "total": req.total_chunks
            })),
        )
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No active extraction session"})),
        )
    }
}

/// Get extraction status
async fn handle_extract_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let session = state.extraction_session.read().await;

    if let Some(ref s) = *session {
        Json(serde_json::json!({
            "sessionId": s.id,
            "chunksReceived": s.chunks_received,
            "totalChunks": s.total_chunks,
            "complete": s.total_chunks.map(|t| s.chunks_received >= t).unwrap_or(false)
        }))
    } else {
        Json(serde_json::json!({
            "sessionId": null,
            "status": "no_active_session"
        }))
    }
}

/// Export extraction data to file
#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub output_path: String,
}

async fn handle_extract_export(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExportRequest>,
) -> impl IntoResponse {
    let session = state.extraction_session.read().await;

    if let Some(ref s) = *session {
        // Flatten all chunks into a single array of instances
        let mut all_instances = Vec::new();
        for chunk in &s.data {
            if let Some(instances) = chunk.as_array() {
                all_instances.extend(instances.iter().cloned());
            }
        }

        tracing::info!("Exporting {} instances to {}", all_instances.len(), req.output_path);

        // Write to file
        let output = serde_json::json!({
            "sessionId": s.id,
            "instanceCount": all_instances.len(),
            "instances": all_instances,
        });

        match std::fs::write(&req.output_path, serde_json::to_string_pretty(&output).unwrap()) {
            Ok(_) => {
                tracing::info!("Export complete: {}", req.output_path);
                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "success": true,
                        "path": req.output_path,
                        "instanceCount": all_instances.len()
                    })),
                )
            }
            Err(e) => {
                tracing::error!("Export failed: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "success": false,
                        "error": e.to_string()
                    })),
                )
            }
        }
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "No extraction data available"
            })),
        )
    }
}

/// Finalize extraction - build proper file tree from chunks
#[derive(Debug, Deserialize)]
pub struct FinalizeRequest {
    pub project_dir: String,
}

async fn handle_extract_finalize(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FinalizeRequest>,
) -> impl IntoResponse {
    let session = state.extraction_session.read().await;

    if session.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "No extraction session active"
            })),
        );
    }

    let session = session.as_ref().unwrap();
    let src_dir = PathBuf::from(&req.project_dir).join("src");

    // Load project config and tree mapping
    let config = load_project_config(&req.project_dir);
    let tree_mapping = get_tree_mapping(&config);
    tracing::info!("Tree mapping loaded: {:?}", tree_mapping);

    // Clear existing src directory before extraction to remove stale files
    if src_dir.exists() {
        tracing::info!("Clearing existing src directory for fresh extraction: {}", src_dir.display());
        for entry in std::fs::read_dir(&src_dir).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    if let Err(e) = std::fs::remove_dir_all(&path) {
                        tracing::warn!("Failed to remove directory {:?}: {}", path, e);
                    }
                } else if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!("Failed to remove file {:?}: {}", path, e);
                }
            }
        }
    }

    // Flatten all chunks into a single array of instances
    let mut all_instances: Vec<serde_json::Value> = Vec::new();
    for chunk in &session.data {
        if let Some(instances) = chunk.as_array() {
            all_instances.extend(instances.iter().cloned());
        }
    }

    tracing::info!("Finalizing {} instances to {}", all_instances.len(), src_dir.display());

    // Create src directory
    let _ = std::fs::create_dir_all(&src_dir);

    // Track which services we've seen to create folders for them
    let mut service_folders: std::collections::HashSet<String> = std::collections::HashSet::new();

    // First pass: build a map from referenceId to disambiguated path
    // This handles duplicate sibling names by appending a suffix
    let mut path_to_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut ref_to_path: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut duplicate_count = 0;

    for inst in &all_instances {
        if let Some(path) = inst.get("path").and_then(|v| v.as_str()) {
            if !path.is_empty() {
                let ref_id = inst.get("referenceId").and_then(|v| v.as_str()).unwrap_or("");
                let count = path_to_count.entry(path.to_string()).or_insert(0);
                *count += 1;

                // If this is a duplicate path, append a suffix
                let disambiguated_path = if *count > 1 {
                    // Use referenceId suffix for disambiguation (first 8 chars)
                    let suffix = if ref_id.len() >= 8 { &ref_id[..8] } else { ref_id };
                    let class_name = inst.get("className").and_then(|v| v.as_str()).unwrap_or("Unknown");
                    tracing::warn!(
                        "Duplicate instance path detected: '{}' ({}). Disambiguating to '{}_{}'",
                        path, class_name, path, suffix
                    );
                    duplicate_count += 1;
                    format!("{}_{}", path, suffix)
                } else {
                    path.to_string()
                };

                if !ref_id.is_empty() {
                    ref_to_path.insert(ref_id.to_string(), disambiguated_path);
                }
            }
        }
    }

    if duplicate_count > 0 {
        tracing::info!("Found {} duplicate instance paths - these have been disambiguated", duplicate_count);
    }

    // Collect all disambiguated paths for container detection
    let all_paths: std::collections::HashSet<String> = ref_to_path.values().cloned().collect();

    // Helper to check if a path has children (is a container)
    let has_children = |path: &str| -> bool {
        let prefix = format!("{}/", path);
        all_paths.iter().any(|p| p.starts_with(&prefix))
    };

    // Helper to normalize package paths (fix duplicated Packages folders)
    let normalize_path = |path: &str| -> String {
        // Fix case variations and duplications like "Packages/Packages" or "packages/Packages"
        let mut normalized = path.to_string();

        // Replace various case-insensitive duplications
        let patterns = [
            ("Packages/Packages/", "Packages/"),
            ("packages/packages/", "packages/"),
            ("Packages/packages/", "Packages/"),
            ("packages/Packages/", "Packages/"),
        ];

        for (from, to) in patterns {
            while normalized.contains(from) {
                normalized = normalized.replace(from, to);
            }
        }

        normalized
    };

    // Write each instance to its own file using the path field
    let mut files_written = 0;
    let mut scripts_written = 0;

    for inst in &all_instances {
        let class_name = inst.get("className").and_then(|v| v.as_str()).unwrap_or("Unknown");

        // Use disambiguated path from ref_to_path map to handle duplicate instance names
        let ref_id = inst.get("referenceId").and_then(|v| v.as_str()).unwrap_or("");
        let inst_path = if !ref_id.is_empty() {
            ref_to_path.get(ref_id).map(|s| s.as_str()).unwrap_or("")
        } else {
            inst.get("path").and_then(|v| v.as_str()).unwrap_or("")
        };
        if inst_path.is_empty() {
            continue;
        }

        // Normalize path to fix package folder duplication
        let inst_path = normalize_path(inst_path);

        // Apply tree mapping to convert DataModel path to filesystem path
        let fs_path = apply_tree_mapping(&inst_path, &tree_mapping);

        // Use mapped path for filesystem operations
        let full_path = src_dir.join(&fs_path);

        // Track service name (first segment of mapped path) for folder creation
        if let Some(service_name) = fs_path.split('/').next() {
            service_folders.insert(service_name.to_string());
        }

        // Create parent directories
        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Check if this instance has children (use normalized path)
        let is_container = has_children(&inst_path);

        // Check if this is a script with source
        let is_script = matches!(class_name, "Script" | "LocalScript" | "ModuleScript");

        if is_script {
            // Extract script source to .luau file
            if let Some(props) = inst.get("properties") {
                if let Some(source) = props.get("Source").and_then(|v| v.get("value")).and_then(|v| v.as_str()) {
                    let extension = match class_name {
                        "Script" => ".server.luau",
                        "LocalScript" => ".client.luau",
                        _ => ".luau",
                    };
                    // Don't use with_extension() - it breaks names with periods like "Br. yellowish orange"
                    let script_path = full_path.to_string_lossy().to_string() + extension;
                    if std::fs::write(&script_path, source).is_ok() {
                        scripts_written += 1;
                    }
                }
            }
        }

        // Write .rbxjson file for all instances (including scripts for metadata)
        // For containers (instances with children), write _meta.rbxjson inside the folder
        // For leaf instances, write as sibling .rbxjson
        let json_path = if is_container {
            // Container: create folder and put _meta.rbxjson inside
            let _ = std::fs::create_dir_all(&full_path);
            full_path.join("_meta.rbxjson")
        } else {
            // Leaf: write as sibling .rbxjson
            // Don't use with_extension() - it breaks names with periods like "Br. yellowish orange"
            PathBuf::from(full_path.to_string_lossy().to_string() + ".rbxjson")
        };

        // Create a clean instance object without source (for scripts)
        let mut clean_inst = inst.clone();
        if is_script {
            if let Some(props) = clean_inst.get_mut("properties") {
                if let Some(obj) = props.as_object_mut() {
                    obj.remove("Source");
                }
            }
        }

        if let Ok(json) = serde_json::to_string_pretty(&clean_inst) {
            if std::fs::write(&json_path, json).is_ok() {
                files_written += 1;
            }
        }
    }

    // Clean up chunk files
    if let Ok(entries) = std::fs::read_dir(&src_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("chunk_") && name.ends_with(".json") {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    // Create service folders even if they're empty
    for service in &service_folders {
        let service_folder = src_dir.join(service);
        // Create the folder if it doesn't exist
        let _ = std::fs::create_dir_all(&service_folder);
    }

    tracing::info!("Finalize complete: {} .rbxjson files, {} .luau scripts, {} services", files_written, scripts_written, service_folders.len());

    // Resume live sync after extraction is complete (with a small delay to let file system settle)
    state.live_sync_paused.store(false, std::sync::atomic::Ordering::Relaxed);
    tracing::info!("Live sync resumed after extraction");

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "filesWritten": files_written,
            "scriptsWritten": scripts_written,
            "totalInstances": all_instances.len()
        })),
    )
}

/// Terrain extraction request
#[derive(Debug, Deserialize)]
pub struct TerrainRequest {
    pub project_dir: String,
    pub session_id: Option<String>,
    pub terrain: serde_json::Value,
    pub batch_index: Option<i32>,
    pub total_batches: Option<i32>,
}

/// Handle terrain data from extraction (supports batched uploads)
async fn handle_extract_terrain(Json(req): Json<TerrainRequest>) -> impl IntoResponse {
    let terrain_dir = PathBuf::from(&req.project_dir).join("src").join("Workspace").join("Terrain");

    // Create terrain directory
    if let Err(e) = std::fs::create_dir_all(&terrain_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Failed to create terrain directory: {}", e)
            })),
        );
    }

    let terrain_file = terrain_dir.join("terrain.rbxjson");
    let batch_index = req.batch_index.unwrap_or(1);
    let total_batches = req.total_batches.unwrap_or(1);

    // For batched uploads, merge with existing data
    let final_terrain = if batch_index == 1 {
        // First batch - use as base
        req.terrain.clone()
    } else {
        // Subsequent batch - merge chunks with existing file
        let existing = std::fs::read_to_string(&terrain_file)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());

        if let Some(mut existing_terrain) = existing {
            // Append new chunks to existing
            if let (Some(existing_chunks), Some(new_chunks)) = (
                existing_terrain.get_mut("chunks").and_then(|c| c.as_array_mut()),
                req.terrain.get("chunks").and_then(|c| c.as_array()),
            ) {
                for chunk in new_chunks {
                    existing_chunks.push(chunk.clone());
                }
            }
            existing_terrain
        } else {
            req.terrain.clone()
        }
    };

    // Write terrain data to file
    let terrain_json = match serde_json::to_string_pretty(&final_terrain) {
        Ok(json) => json,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to serialize terrain: {}", e)
                })),
            );
        }
    };

    if let Err(e) = std::fs::write(&terrain_file, terrain_json) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Failed to write terrain file: {}", e)
            })),
        );
    }

    let chunk_count = final_terrain.get("chunks")
        .and_then(|c| c.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    tracing::info!("Terrain batch {}/{} saved: {} total chunks", batch_index, total_batches, chunk_count);

    tracing::info!("Terrain saved: {} chunks to {}", chunk_count, terrain_file.display());

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "chunksWritten": chunk_count,
            "path": terrain_file.to_string_lossy()
        })),
    )
}

/// Sync command request
#[derive(Debug, Deserialize)]
pub struct SyncCommandRequest {
    pub command: String,
    pub payload: serde_json::Value,
}

/// Handle sync command - sends to plugin and waits for response
async fn handle_sync_command(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SyncCommandRequest>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4();

    // Create response channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    {
        let mut channels = state.response_channels.write().await;
        channels.insert(request_id, tx);
    }

    // Queue request to plugin
    let plugin_request = PluginRequest {
        id: request_id,
        command: req.command.clone(),
        payload: req.payload,
    };

    {
        let mut queue = state.request_queue.lock().await;
        queue.push_back(plugin_request);
    }
    let _ = state.trigger.send(());

    tracing::info!("Sent sync command: {} ({})", req.command, request_id);

    // Wait for response with timeout
    let timeout = tokio::time::Duration::from_secs(30);
    let result = tokio::time::timeout(timeout, rx.recv()).await;

    // Clean up channel
    {
        let mut channels = state.response_channels.write().await;
        channels.remove(&request_id);
    }

    match result {
        Ok(Some(response)) => {
            tracing::info!("Received response for {}: success={}", request_id, response.success);
            (StatusCode::OK, Json(serde_json::to_value(&response).unwrap()))
        }
        Ok(None) => {
            tracing::warn!("Channel closed for {}", request_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Channel closed"})),
            )
        }
        Err(_) => {
            tracing::warn!("Timeout waiting for response: {}", request_id);
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(serde_json::json!({"error": "Timeout waiting for plugin response"})),
            )
        }
    }
}

/// Sync batch request
#[derive(Debug, Deserialize)]
pub struct SyncBatchRequest {
    pub operations: Vec<serde_json::Value>,
}

/// Handle sync batch - sends batch of operations to plugin
async fn handle_sync_batch(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SyncBatchRequest>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4();

    // Create response channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    {
        let mut channels = state.response_channels.write().await;
        channels.insert(request_id, tx);
    }

    // Queue batch request to plugin
    let plugin_request = PluginRequest {
        id: request_id,
        command: "sync:batch".to_string(),
        payload: serde_json::json!({
            "operations": req.operations
        }),
    };

    {
        let mut queue = state.request_queue.lock().await;
        queue.push_back(plugin_request);
    }
    let _ = state.trigger.send(());

    tracing::info!("Sent sync batch with {} operations ({})", req.operations.len(), request_id);

    // Wait for response with longer timeout for batch operations
    let timeout = tokio::time::Duration::from_secs(300); // 5 minutes for large batches
    let result = tokio::time::timeout(timeout, rx.recv()).await;

    // Clean up channel
    {
        let mut channels = state.response_channels.write().await;
        channels.remove(&request_id);
    }

    match result {
        Ok(Some(response)) => {
            tracing::info!("Batch complete for {}: success={}", request_id, response.success);
            (StatusCode::OK, Json(serde_json::to_value(&response).unwrap()))
        }
        Ok(None) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Channel closed"})),
            )
        }
        Err(_) => {
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(serde_json::json!({"error": "Timeout waiting for plugin response"})),
            )
        }
    }
}

/// Sync changes from Studio back to files
#[derive(Debug, Deserialize)]
pub struct SyncFromStudioRequest {
    pub operations: Vec<StudioChangeOperation>,
    #[serde(rename = "projectDir")]
    pub project_dir: String,
}

#[derive(Debug, Deserialize)]
pub struct StudioChangeOperation {
    #[serde(rename = "type")]
    pub change_type: String,  // "create", "modify", "delete"
    pub path: String,
    #[serde(rename = "className")]
    pub class_name: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// Handle changes from Studio and write them to files
async fn handle_sync_from_studio(Json(req): Json<SyncFromStudioRequest>) -> impl IntoResponse {
    tracing::info!("handle_sync_from_studio called with {} operations", req.operations.len());
    for (i, op) in req.operations.iter().enumerate() {
        tracing::info!("  Op {}: type={}, path={}, className={:?}, has_data={}",
            i, op.change_type, op.path, op.class_name, op.data.is_some());
    }
    let src_dir = PathBuf::from(&req.project_dir).join("src");

    if !src_dir.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "Source directory does not exist"
            })),
        );
    }

    // Load project config and tree mapping
    let config = load_project_config(&req.project_dir);
    let tree_mapping = get_tree_mapping(&config);

    let mut files_written = 0;
    let mut errors: Vec<String> = Vec::new();

    for op in &req.operations {
        // Convert instance path to file path with tree mapping
        let inst_path = &op.path;
        let fs_path = apply_tree_mapping(inst_path, &tree_mapping);
        let full_path = src_dir.join(&fs_path);

        match op.change_type.as_str() {
            "delete" => {
                // Try to delete both .luau and .rbxjson files
                let luau_extensions = [".server.luau", ".client.luau", ".luau"];
                let mut deleted_any = false;
                for ext in luau_extensions {
                    let script_path = format!("{}{}", full_path.to_string_lossy(), ext);
                    if std::fs::remove_file(&script_path).is_ok() {
                        deleted_any = true;
                        tracing::info!("Studio sync: deleted {}", script_path);
                    }
                }
                let json_path = format!("{}.rbxjson", full_path.to_string_lossy());
                if std::fs::remove_file(&json_path).is_ok() {
                    deleted_any = true;
                    tracing::info!("Studio sync: deleted {}", json_path);
                }

                // Try to delete as a directory (for Folder instances)
                if full_path.is_dir() {
                    if std::fs::remove_dir_all(&full_path).is_ok() {
                        deleted_any = true;
                        tracing::info!("Studio sync: deleted folder {:?}", full_path);
                    }
                }

                if deleted_any {
                    files_written += 1;
                }
            }
            "create" | "modify" => {
                if let Some(data) = &op.data {
                    // Ensure parent directory exists
                    if let Some(parent) = full_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }

                    // Check if this is a script with source
                    let class_name = op.class_name.as_deref()
                        .or_else(|| data.get("className").and_then(|v| v.as_str()))
                        .unwrap_or("");

                    let is_script = matches!(class_name, "Script" | "LocalScript" | "ModuleScript");
                    tracing::info!("Processing {} - class_name: '{}', is_script: {}, data: {:?}", inst_path, class_name, is_script, data);

                    if is_script {
                        // Extract script source - try multiple formats
                        // Format 1: data.source (from ChangeTracker)
                        // Format 2: data.properties.Source.value (from full extraction)
                        let source = data.get("source")
                            .and_then(|v| v.as_str())
                            .or_else(|| {
                                data.get("properties")
                                    .and_then(|p| p.get("Source"))
                                    .and_then(|s| s.get("value"))
                                    .and_then(|v| v.as_str())
                            });

                        tracing::debug!("Source extraction result: {:?}", source.map(|s| s.len()));
                        if let Some(source) = source {
                            let extension = match class_name {
                                "Script" => ".server.luau",
                                "LocalScript" => ".client.luau",
                                _ => ".luau",
                            };
                            let script_path = format!("{}{}", full_path.to_string_lossy(), extension);

                            match std::fs::write(&script_path, source) {
                                Ok(_) => {
                                    tracing::info!("Studio sync: wrote {}", script_path);
                                    files_written += 1;
                                }
                                Err(e) => {
                                    errors.push(format!("Failed to write {}: {}", script_path, e));
                                }
                            }
                        }
                    }

                    // Write .rbxjson for non-source properties
                    let mut clean_data = data.clone();
                    if is_script {
                        // Remove source from both formats
                        if let Some(obj) = clean_data.as_object_mut() {
                            obj.remove("source");
                        }
                        if let Some(props) = clean_data.get_mut("properties") {
                            if let Some(obj) = props.as_object_mut() {
                                obj.remove("Source");
                            }
                        }
                    }

                    let json_path = format!("{}.rbxjson", full_path.to_string_lossy());
                    if let Ok(json) = serde_json::to_string_pretty(&clean_data) {
                        match std::fs::write(&json_path, json) {
                            Ok(_) => {
                                files_written += 1;
                            }
                            Err(e) => {
                                errors.push(format!("Failed to write {}: {}", json_path, e));
                            }
                        }
                    }
                }
            }
            _ => {
                errors.push(format!("Unknown change type: {}", op.change_type));
            }
        }
    }

    tracing::info!("Studio sync complete: {} files written, {} errors", files_written, errors.len());

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": errors.is_empty(),
            "filesWritten": files_written,
            "errors": errors
        })),
    )
}

/// Read file tree for sync - returns all instances from project directory
#[derive(Debug, Deserialize)]
pub struct ReadTreeRequest {
    pub project_dir: String,
}

async fn handle_sync_read_tree(Json(req): Json<ReadTreeRequest>) -> impl IntoResponse {
    let src_dir = PathBuf::from(&req.project_dir).join("src");

    if !src_dir.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "Source directory does not exist"
            })),
        );
    }

    // Recursively read all .rbxjson files
    let mut instances: Vec<serde_json::Value> = Vec::new();
    let mut scripts: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    fn walk_dir(
        dir: &std::path::Path,
        base: &std::path::Path,
        instances: &mut Vec<serde_json::Value>,
        scripts: &mut std::collections::HashMap<String, String>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_dir(&path, base, instances, scripts);
                } else if let Some(ext) = path.extension() {
                    if ext == "rbxjson" {
                        // Skip terrain.rbxjson - it has different format (terrain chunk data, not instance data)
                        let filename = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
                        if filename == "terrain.rbxjson" {
                            tracing::debug!("Skipping terrain file: {:?}", path);
                            continue;
                        }
                        // Read instance JSON
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(mut inst) = serde_json::from_str::<serde_json::Value>(&content) {
                                // Derive path from file system if not present in JSON
                                let rel_path = path.strip_prefix(base).unwrap_or(&path);
                                let path_str = rel_path.to_string_lossy().to_string();
                                // Convert file path to instance path:
                                // e.g., "Workspace/MyPart.rbxjson" -> "Workspace/MyPart"
                                // e.g., "Workspace/MyPart/_meta.rbxjson" -> "Workspace/MyPart"
                                let is_meta = path_str.ends_with("/_meta.rbxjson") || path_str.ends_with("\\_meta.rbxjson");
                                let inst_path = if is_meta {
                                    // _meta.rbxjson represents the parent folder
                                    path_str.replace("/_meta.rbxjson", "").replace("\\_meta.rbxjson", "")
                                } else {
                                    path_str.replace(".rbxjson", "")
                                };
                                if path_str.contains("_meta") {
                                    tracing::info!("DEBUG: path_str='{}', is_meta={}, inst_path='{}'", path_str, is_meta, inst_path);
                                }

                                // Set path from file location (used for tracking, not naming)
                                if let Some(obj) = inst.as_object_mut() {
                                    // Always set path from file location
                                    obj.insert("path".to_string(), serde_json::Value::String(inst_path.clone()));

                                    // Only set name if not provided in JSON
                                    if !obj.contains_key("name") {
                                        if let Some(name) = inst_path.rsplit('/').next() {
                                            obj.insert("name".to_string(), serde_json::Value::String(name.to_string()));
                                        }
                                    }
                                }
                                instances.push(inst);
                            }
                        }
                    } else if ext == "luau" {
                        // Read script source
                        let rel_path = path.strip_prefix(base).unwrap_or(&path);
                        let path_str = rel_path.to_string_lossy().to_string();
                        // Keep '/' as delimiter (matches instance path format)
                        // e.g., "ServerScriptService/MyScript.server.luau" -> "ServerScriptService/MyScript"
                        let inst_path = path_str
                            .trim_end_matches(".server.luau")
                            .trim_end_matches(".client.luau")
                            .trim_end_matches(".luau")
                            .to_string();
                        if let Ok(source) = std::fs::read_to_string(&path) {
                            scripts.insert(inst_path, source);
                        }
                    }
                }
            }
        }
    }

    walk_dir(&src_dir, &src_dir, &mut instances, &mut scripts);

    // Merge script sources into their instance data
    for inst in &mut instances {
        if let Some(path) = inst.get("path").and_then(|v| v.as_str()) {
            if let Some(source) = scripts.get(path) {
                // Add or update Source property
                if let Some(props) = inst.get_mut("properties") {
                    if let Some(obj) = props.as_object_mut() {
                        obj.insert("Source".to_string(), serde_json::json!({
                            "type": "string",
                            "value": source
                        }));
                    }
                }
            }
        }
    }

    tracing::info!("Read {} instances from {}", instances.len(), src_dir.display());

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "instances": instances,
            "count": instances.len()
        })),
    )
}

/// Read terrain data for sync
async fn handle_sync_read_terrain(Json(req): Json<ReadTreeRequest>) -> impl IntoResponse {
    let terrain_file = PathBuf::from(&req.project_dir)
        .join("src")
        .join("Workspace")
        .join("Terrain")
        .join("terrain.rbxjson");

    if !terrain_file.exists() {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "hasTerrain": false
            })),
        );
    }

    match std::fs::read_to_string(&terrain_file) {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(terrain_data) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "success": true,
                        "hasTerrain": true,
                        "terrain": terrain_data
                    })),
                ),
                Err(e) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "success": false,
                        "error": format!("Failed to parse terrain data: {}", e)
                    })),
                ),
            }
        }
        Err(e) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Failed to read terrain file: {}", e)
            })),
        ),
    }
}

/// Request to check pending changes count
#[derive(Debug, Deserialize)]
pub struct PendingChangesRequest {
    pub project_dir: String,
}

/// Handle pending changes request - returns count of files waiting to sync
async fn handle_sync_pending_changes(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PendingChangesRequest>,
) -> impl IntoResponse {
    // Check pending changes in file watcher
    let file_watcher = state.file_watcher_state.read().await;

    // Filter pending changes by project directory
    let src_prefix = PathBuf::from(&req.project_dir).join("src");
    let count = file_watcher.pending_changes.iter()
        .filter(|(path, _)| path.starts_with(&src_prefix))
        .count();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "count": count
        })),
    )
}

/// Request for incremental sync - returns only files changed since last sync
#[derive(Debug, Deserialize)]
pub struct IncrementalSyncRequest {
    pub project_dir: String,
    /// If true, mark current time as last sync (call after successful sync)
    #[serde(default)]
    pub mark_synced: bool,
}

/// Handle incremental sync - returns only files modified since last sync
async fn handle_sync_incremental(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IncrementalSyncRequest>,
) -> impl IntoResponse {
    let src_dir = PathBuf::from(&req.project_dir).join("src");

    if !src_dir.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "Source directory does not exist"
            })),
        );
    }

    // Get last sync time for this project
    let last_sync = {
        let sync_state = state.sync_state.read().await;
        sync_state.get(&req.project_dir).copied()
    };

    // If marking as synced, update the sync time and return empty
    if req.mark_synced {
        let mut sync_state = state.sync_state.write().await;
        sync_state.insert(req.project_dir.clone(), std::time::SystemTime::now());
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "instances": [],
                "count": 0,
                "full_sync": false,
                "marked_synced": true
            })),
        );
    }

    // Recursively read files, filtering by modification time
    let mut instances: Vec<serde_json::Value> = Vec::new();
    let mut scripts: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut files_checked = 0usize;
    let mut files_modified = 0usize;

    fn walk_dir_incremental(
        dir: &std::path::Path,
        base: &std::path::Path,
        instances: &mut Vec<serde_json::Value>,
        scripts: &mut std::collections::HashMap<String, String>,
        last_sync: Option<std::time::SystemTime>,
        files_checked: &mut usize,
        files_modified: &mut usize,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_dir_incremental(&path, base, instances, scripts, last_sync, files_checked, files_modified);
                } else if let Some(ext) = path.extension() {
                    *files_checked += 1;

                    // Check if file was modified since last sync
                    let is_modified = if let Some(last_sync_time) = last_sync {
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            if let Ok(modified) = metadata.modified() {
                                modified > last_sync_time
                            } else {
                                true // Can't get modification time, include it
                            }
                        } else {
                            true // Can't get metadata, include it
                        }
                    } else {
                        true // No last sync, include all files
                    };

                    if !is_modified {
                        continue;
                    }

                    *files_modified += 1;

                    if ext == "rbxjson" {
                        let filename = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
                        if filename == "terrain.rbxjson" {
                            continue;
                        }

                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(mut inst) = serde_json::from_str::<serde_json::Value>(&content) {
                                let rel_path = path.strip_prefix(base).unwrap_or(&path);
                                let path_str = rel_path.to_string_lossy().to_string();
                                let is_meta = path_str.ends_with("/_meta.rbxjson") || path_str.ends_with("\\_meta.rbxjson");
                                let inst_path = if is_meta {
                                    path_str.replace("/_meta.rbxjson", "").replace("\\_meta.rbxjson", "")
                                } else {
                                    path_str.replace(".rbxjson", "")
                                };

                                if let Some(obj) = inst.as_object_mut() {
                                    obj.insert("path".to_string(), serde_json::Value::String(inst_path.clone()));
                                    if !obj.contains_key("name") {
                                        if let Some(name) = inst_path.rsplit('/').next() {
                                            obj.insert("name".to_string(), serde_json::Value::String(name.to_string()));
                                        }
                                    }
                                }
                                instances.push(inst);
                            }
                        }
                    } else if ext == "luau" {
                        let rel_path = path.strip_prefix(base).unwrap_or(&path);
                        let path_str = rel_path.to_string_lossy().to_string();
                        let inst_path = path_str
                            .trim_end_matches(".server.luau")
                            .trim_end_matches(".client.luau")
                            .trim_end_matches(".luau")
                            .to_string();
                        if let Ok(source) = std::fs::read_to_string(&path) {
                            scripts.insert(inst_path, source);
                        }
                    }
                }
            }
        }
    }

    walk_dir_incremental(&src_dir, &src_dir, &mut instances, &mut scripts, last_sync, &mut files_checked, &mut files_modified);

    // Merge script sources into their instance data
    for inst in &mut instances {
        if let Some(path) = inst.get("path").and_then(|v| v.as_str()) {
            if let Some(source) = scripts.get(path) {
                if let Some(props) = inst.get_mut("properties") {
                    if let Some(obj) = props.as_object_mut() {
                        obj.insert("Source".to_string(), serde_json::json!({
                            "type": "string",
                            "value": source
                        }));
                    }
                }
            }
        }
    }

    // Handle scripts that don't have an .rbxjson (standalone scripts)
    let instance_paths: std::collections::HashSet<String> = instances.iter()
        .filter_map(|inst| inst.get("path").and_then(|v| v.as_str()).map(String::from))
        .collect();

    for (script_path, source) in &scripts {
        if !instance_paths.contains(script_path) {
            // Determine script type from path
            let class_name = if script_path.ends_with(".server") || script_path.contains(".server/") {
                "Script"
            } else if script_path.ends_with(".client") || script_path.contains(".client/") {
                "LocalScript"
            } else {
                "ModuleScript"
            };

            let instance_name = script_path.rsplit('/').next().unwrap_or(script_path);

            instances.push(serde_json::json!({
                "className": class_name,
                "name": instance_name,
                "path": script_path,
                "properties": {
                    "Source": {
                        "type": "string",
                        "value": source
                    }
                }
            }));
        }
    }

    let full_sync = last_sync.is_none();

    tracing::info!(
        "Incremental sync: checked {} files, {} modified (full_sync: {})",
        files_checked, files_modified, full_sync
    );

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "instances": instances,
            "count": instances.len(),
            "full_sync": full_sync,
            "files_checked": files_checked,
            "files_modified": files_modified
        })),
    )
}

// ============================================================================
// Diff Endpoints
// ============================================================================

/// Request to get Studio paths
#[derive(Debug, Deserialize)]
pub struct StudioPathsRequest {
    #[serde(default)]
    pub services: Option<Vec<String>>,
}

/// Single path entry from Studio
#[derive(Debug, Serialize, Deserialize)]
pub struct StudioPathEntry {
    pub path: String,
    #[serde(rename = "className")]
    pub class_name: String,
    pub name: String,
}

/// Response from studio:paths command
#[derive(Debug, Deserialize)]
pub struct StudioPathsResponse {
    pub success: bool,
    pub paths: Vec<StudioPathEntry>,
    pub count: usize,
}

/// Handle studio paths request - gets all instance paths from Studio via plugin
async fn handle_studio_paths(
    State(state): State<Arc<AppState>>,
    Json(_req): Json<StudioPathsRequest>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4();

    // Create response channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    {
        let mut channels = state.response_channels.write().await;
        channels.insert(request_id, tx);
    }

    // Queue request to plugin
    let plugin_request = PluginRequest {
        id: request_id,
        command: "studio:paths".to_string(),
        payload: serde_json::json!({}),
    };

    {
        let mut queue = state.request_queue.lock().await;
        queue.push_back(plugin_request);
    }
    let _ = state.trigger.send(());

    tracing::info!("Requesting Studio paths ({})", request_id);

    // Wait for response with timeout (60s for large games)
    let timeout = tokio::time::Duration::from_secs(60);
    let result = tokio::time::timeout(timeout, rx.recv()).await;

    // Clean up channel
    {
        let mut channels = state.response_channels.write().await;
        channels.remove(&request_id);
    }

    match result {
        Ok(Some(response)) => {
            tracing::info!("Received Studio paths: success={}", response.success);
            (StatusCode::OK, Json(response.data))
        }
        Ok(None) => {
            tracing::warn!("Channel closed for {}", request_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"success": false, "error": "Channel closed"})),
            )
        }
        Err(_) => {
            tracing::warn!("Timeout waiting for Studio paths: {}", request_id);
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(serde_json::json!({"success": false, "error": "Timeout waiting for plugin response"})),
            )
        }
    }
}

/// Diff request
#[derive(Debug, Deserialize)]
pub struct DiffRequest {
    pub project_dir: String,
}

/// Single diff entry
#[derive(Debug, Serialize)]
pub struct DiffEntry {
    pub path: String,
    #[serde(rename = "className")]
    pub class_name: String,
}

/// Diff result
#[derive(Debug, Serialize)]
pub struct DiffResult {
    pub added: Vec<DiffEntry>,      // In files, not in Studio (would be created)
    pub removed: Vec<DiffEntry>,    // In Studio, not in files (would be deleted)
    pub common: usize,              // In both
}

/// Handle diff request - compares files with Studio
async fn handle_diff(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DiffRequest>,
) -> impl IntoResponse {
    // 1. Read file tree
    let src_dir = PathBuf::from(&req.project_dir).join("src");
    if !src_dir.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "Source directory does not exist"
            })),
        );
    }

    // Collect file paths
    let mut file_paths: HashSet<String> = HashSet::new();
    let mut file_classes: HashMap<String, String> = HashMap::new();

    fn collect_file_paths(
        dir: &std::path::Path,
        base: &std::path::Path,
        paths: &mut HashSet<String>,
        classes: &mut HashMap<String, String>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_file_paths(&path, base, paths, classes);
                } else if let Some(ext) = path.extension() {
                    if ext == "rbxjson" {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(inst) = serde_json::from_str::<serde_json::Value>(&content) {
                                let rel_path = path.strip_prefix(base).unwrap_or(&path);
                                let path_str = rel_path.to_string_lossy().to_string();
                                let is_meta = path_str.ends_with("/_meta.rbxjson") || path_str.ends_with("\\_meta.rbxjson");
                                let inst_path = if is_meta {
                                    path_str.replace("/_meta.rbxjson", "").replace("\\_meta.rbxjson", "")
                                } else {
                                    path_str.replace(".rbxjson", "")
                                };
                                // Normalize path separators
                                let inst_path = inst_path.replace('\\', "/");
                                paths.insert(inst_path.clone());
                                if let Some(class) = inst.get("className").and_then(|v| v.as_str()) {
                                    classes.insert(inst_path, class.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    collect_file_paths(&src_dir, &src_dir, &mut file_paths, &mut file_classes);
    tracing::info!("Read {} file paths from {}", file_paths.len(), src_dir.display());

    // 2. Get Studio paths via plugin
    let request_id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::unbounded_channel();
    {
        let mut channels = state.response_channels.write().await;
        channels.insert(request_id, tx);
    }

    let plugin_request = PluginRequest {
        id: request_id,
        command: "studio:paths".to_string(),
        payload: serde_json::json!({}),
    };

    {
        let mut queue = state.request_queue.lock().await;
        queue.push_back(plugin_request);
    }
    let _ = state.trigger.send(());

    let timeout = tokio::time::Duration::from_secs(60);
    let result = tokio::time::timeout(timeout, rx.recv()).await;

    {
        let mut channels = state.response_channels.write().await;
        channels.remove(&request_id);
    }

    let studio_response = match result {
        Ok(Some(response)) if response.success => response.data,
        Ok(Some(response)) => {
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": false,
                    "error": response.error.unwrap_or_else(|| "Plugin returned error".to_string())
                })),
            );
        }
        Ok(None) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"success": false, "error": "Channel closed"})),
            );
        }
        Err(_) => {
            return (
                StatusCode::GATEWAY_TIMEOUT,
                Json(serde_json::json!({"success": false, "error": "Timeout waiting for Studio paths"})),
            );
        }
    };

    // Parse studio paths
    let mut studio_paths: HashSet<String> = HashSet::new();
    let mut studio_classes: HashMap<String, String> = HashMap::new();

    if let Some(paths) = studio_response.get("paths").and_then(|v| v.as_array()) {
        for entry in paths {
            if let Some(path) = entry.get("path").and_then(|v| v.as_str()) {
                studio_paths.insert(path.to_string());
                if let Some(class) = entry.get("className").and_then(|v| v.as_str()) {
                    studio_classes.insert(path.to_string(), class.to_string());
                }
            }
        }
    }

    tracing::info!("Got {} Studio paths", studio_paths.len());

    // 3. Compute diff
    let added: Vec<DiffEntry> = file_paths
        .difference(&studio_paths)
        .map(|path| DiffEntry {
            path: path.clone(),
            class_name: file_classes.get(path).cloned().unwrap_or_default(),
        })
        .collect();

    let removed: Vec<DiffEntry> = studio_paths
        .difference(&file_paths)
        .map(|path| DiffEntry {
            path: path.clone(),
            class_name: studio_classes.get(path).cloned().unwrap_or_default(),
        })
        .collect();

    let common = file_paths.intersection(&studio_paths).count();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "added": added,
            "removed": removed,
            "common": common,
            "file_count": file_paths.len(),
            "studio_count": studio_paths.len()
        })),
    )
}

// ============================================================================
// Git Endpoints
// ============================================================================

/// Git project directory request (shared by all git endpoints)
#[derive(Debug, Deserialize)]
pub struct GitProjectRequest {
    pub project_dir: String,
}

/// Git status request
#[derive(Debug, Deserialize)]
pub struct GitStatusRequest {
    pub project_dir: String,
}

/// Handle git status request
async fn handle_git_status(Json(req): Json<GitStatusRequest>) -> impl IntoResponse {
    let project_path = PathBuf::from(&req.project_dir);

    match git::get_status(&project_path) {
        Ok(status) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "data": status
            })),
        ),
        Err(e) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": false,
                "error": e
            })),
        ),
    }
}

/// Git log request
#[derive(Debug, Deserialize)]
pub struct GitLogRequest {
    pub project_dir: String,
    pub limit: Option<usize>,
}

/// Handle git log request
async fn handle_git_log(Json(req): Json<GitLogRequest>) -> impl IntoResponse {
    let project_path = PathBuf::from(&req.project_dir);
    let limit = req.limit.unwrap_or(5);

    match git::get_log(&project_path, limit) {
        Ok(commits) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "data": commits
            })),
        ),
        Err(e) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": false,
                "error": e
            })),
        ),
    }
}

/// Git commit request
#[derive(Debug, Deserialize)]
pub struct GitCommitRequest {
    pub project_dir: String,
    pub message: String,
    pub add_all: Option<bool>,
}

/// Handle git commit request
async fn handle_git_commit(Json(req): Json<GitCommitRequest>) -> impl IntoResponse {
    let project_path = PathBuf::from(&req.project_dir);
    let add_all = req.add_all.unwrap_or(true);

    match git::commit(&project_path, &req.message, add_all) {
        Ok(output) => {
            tracing::info!("Git commit successful in {}", req.project_dir);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "data": output
                })),
            )
        }
        Err(e) => {
            tracing::warn!("Git commit failed: {}", e);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": false,
                    "error": e
                })),
            )
        }
    }
}

/// Handle git init request
async fn handle_git_init(Json(req): Json<GitProjectRequest>) -> impl IntoResponse {
    let project_path = PathBuf::from(&req.project_dir);

    match git::init(&project_path) {
        Ok(output) => {
            tracing::info!("Git init successful in {}", req.project_dir);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "data": output
                })),
            )
        }
        Err(e) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": false,
                "error": e
            })),
        ),
    }
}

// =============================================================================
// Test Runner Endpoints
// =============================================================================

/// Response from test operations
#[derive(Debug, Serialize, Deserialize)]
pub struct TestConsoleMessage {
    pub message: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: f64,
}

/// Test status response
#[derive(Debug, Serialize)]
pub struct TestStatusResponse {
    pub capturing: bool,
    pub output: Vec<TestConsoleMessage>,
    pub total_messages: usize,
}

/// Start test capture - tells plugin to start capturing console output
async fn handle_test_start(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Send command to plugin to start capture
    let request_id = Uuid::new_v4();
    let request = PluginRequest {
        id: request_id,
        command: "test:start".to_string(),
        payload: serde_json::json!({}),
    };

    // Create response channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.response_channels.write().await.insert(request_id, tx);

    // Queue the request
    state.request_queue.lock().await.push_back(request);
    state.trigger.send(()).ok();

    // Wait for response with timeout
    let timeout = tokio::time::Duration::from_secs(30);
    match tokio::time::timeout(timeout, rx.recv()).await {
        Ok(Some(response)) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": response.success,
                    "message": response.data.get("message").and_then(|v| v.as_str()).unwrap_or("Capture started")
                })),
            )
        }
        Ok(None) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "Channel closed unexpectedly"
                })),
            )
        }
        Err(_) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::REQUEST_TIMEOUT,
                Json(serde_json::json!({
                    "success": false,
                    "error": "Plugin response timeout - make sure Studio is connected"
                })),
            )
        }
    }
}

/// Get current test capture status and output
async fn handle_test_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Send command to plugin to get current output
    let request_id = Uuid::new_v4();
    let request = PluginRequest {
        id: request_id,
        command: "test:output".to_string(),
        payload: serde_json::json!({}),
    };

    // Create response channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.response_channels.write().await.insert(request_id, tx);

    // Queue the request
    state.request_queue.lock().await.push_back(request);
    state.trigger.send(()).ok();

    // Wait for response with timeout
    let timeout = tokio::time::Duration::from_secs(10);
    match tokio::time::timeout(timeout, rx.recv()).await {
        Ok(Some(response)) => {
            state.response_channels.write().await.remove(&request_id);
            (StatusCode::OK, Json(response.data))
        }
        Ok(None) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "capturing": false,
                    "output": [],
                    "totalMessages": 0,
                    "error": "Channel closed"
                })),
            )
        }
        Err(_) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::REQUEST_TIMEOUT,
                Json(serde_json::json!({
                    "capturing": false,
                    "output": [],
                    "totalMessages": 0,
                    "error": "Plugin response timeout"
                })),
            )
        }
    }
}

/// Stop test capture and return all captured output
async fn handle_test_stop(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Send command to plugin to stop capture
    let request_id = Uuid::new_v4();
    let request = PluginRequest {
        id: request_id,
        command: "test:stop".to_string(),
        payload: serde_json::json!({}),
    };

    // Create response channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.response_channels.write().await.insert(request_id, tx);

    // Queue the request
    state.request_queue.lock().await.push_back(request);
    state.trigger.send(()).ok();

    // Wait for response with timeout
    let timeout = tokio::time::Duration::from_secs(30);
    match tokio::time::timeout(timeout, rx.recv()).await {
        Ok(Some(response)) => {
            state.response_channels.write().await.remove(&request_id);
            (StatusCode::OK, Json(response.data))
        }
        Ok(None) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "output": [],
                    "totalMessages": 0,
                    "error": "Channel closed"
                })),
            )
        }
        Err(_) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::REQUEST_TIMEOUT,
                Json(serde_json::json!({
                    "success": false,
                    "output": [],
                    "totalMessages": 0,
                    "error": "Plugin response timeout"
                })),
            )
        }
    }
}

// ============================================================================
// Console Streaming Endpoints (for E2E Testing Mode)
// ============================================================================

/// Request to push console message(s) from plugin
#[derive(Debug, Deserialize)]
struct ConsolePushRequest {
    messages: Vec<ConsoleMessage>,
}

/// Push console messages from plugin to server
async fn handle_console_push(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConsolePushRequest>,
) -> impl IntoResponse {
    let mut buffer = state.console_buffer.write().await;
    let count = req.messages.len();

    for msg in req.messages {
        // Broadcast to any active subscribers
        let _ = state.console_tx.send(msg.clone());

        // Add to buffer (ring buffer behavior)
        if buffer.len() >= CONSOLE_BUFFER_SIZE {
            buffer.pop_front();
        }
        buffer.push_back(msg);
    }

    Json(serde_json::json!({
        "success": true,
        "received": count
    }))
}

/// Get console message history
async fn handle_console_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ConsoleHistoryQuery>,
) -> impl IntoResponse {
    let buffer = state.console_buffer.read().await;
    let limit = params.limit.unwrap_or(100).min(CONSOLE_BUFFER_SIZE);

    // Get last N messages
    let messages: Vec<&ConsoleMessage> = buffer.iter().rev().take(limit).collect();
    let messages: Vec<&ConsoleMessage> = messages.into_iter().rev().collect();

    Json(serde_json::json!({
        "messages": messages,
        "total": buffer.len()
    }))
}

#[derive(Debug, Deserialize)]
struct ConsoleHistoryQuery {
    limit: Option<usize>,
}

/// Subscribe to console messages via Server-Sent Events
async fn handle_console_subscribe(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    use axum::response::sse::{Event, Sse};
    use std::convert::Infallible;

    let mut rx = state.console_tx.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let json = serde_json::to_string(&msg).unwrap_or_default();
                    yield Ok::<_, Infallible>(Event::default().data(json));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Client fell behind, continue
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive")
    )
}

// ============================================================================
// Run Code Endpoint
// ============================================================================

/// Request structure for running code
#[derive(Debug, Deserialize)]
struct RunCodeRequest {
    code: String,
}

/// Run arbitrary Luau code in Studio (for MCP integration)
async fn handle_run_code(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunCodeRequest>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4();
    tracing::info!("run:code request {} - queuing command", request_id);
    let request = PluginRequest {
        id: request_id,
        command: "run:code".to_string(),
        payload: serde_json::json!({
            "code": req.code
        }),
    };

    // Create response channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.response_channels.write().await.insert(request_id, tx);

    // Queue the request
    let queue_len = {
        let mut queue = state.request_queue.lock().await;
        queue.push_back(request);
        queue.len()
    };
    tracing::info!("run:code request {} - queued (queue length: {})", request_id, queue_len);
    state.trigger.send(()).ok();

    // Wait for response with timeout
    let timeout = tokio::time::Duration::from_secs(30);
    match tokio::time::timeout(timeout, rx.recv()).await {
        Ok(Some(response)) => {
            state.response_channels.write().await.remove(&request_id);
            (StatusCode::OK, Json(serde_json::json!({
                "success": response.success,
                "output": response.data.get("output").and_then(|v| v.as_str()).unwrap_or(""),
                "error": response.error
            })))
        }
        Ok(None) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "output": null,
                    "error": "Channel closed"
                })),
            )
        }
        Err(_) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::REQUEST_TIMEOUT,
                Json(serde_json::json!({
                    "success": false,
                    "output": null,
                    "error": "Plugin response timeout"
                })),
            )
        }
    }
}

/// Start the server
pub async fn run_server(config: ServerConfig) -> anyhow::Result<()> {
    let state = AppState::new();
    let router = create_router(state.clone());

    // Start background task to process file changes for live sync
    let state_for_watcher = state.clone();
    tokio::spawn(async move {
        process_file_changes(state_for_watcher).await;
    });

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("RbxSync server listening on {}", addr);
    axum::serve(listener, router).await?;

    Ok(())
}

/// Background task to process file changes and send sync commands to the plugin
async fn process_file_changes(state: Arc<AppState>) {
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    let mut pending: HashMap<PathBuf, (file_watcher::FileChange, Instant)> = HashMap::new();
    let debounce_duration = Duration::from_millis(300);

    loop {
        // Try to receive file changes
        {
            let mut rx = state.file_change_rx.lock().await;
            while let Ok(change) = rx.try_recv() {
                // Debounce: update pending changes
                pending.insert(change.path.clone(), (change, Instant::now()));
            }
        }

        // Process changes that have passed debounce period
        let now = Instant::now();
        let mut ready_changes: Vec<file_watcher::FileChange> = Vec::new();

        pending.retain(|_, (change, time)| {
            if now.duration_since(*time) >= debounce_duration {
                ready_changes.push(change.clone());
                false
            } else {
                true
            }
        });

        // Send ready changes to plugin (skip if live sync is paused during extraction)
        if !ready_changes.is_empty() {
            // Check if live sync is paused (during extraction)
            if state.live_sync_paused.load(std::sync::atomic::Ordering::Relaxed) {
                tracing::debug!("Live sync paused, skipping {} file changes", ready_changes.len());
                continue;
            }

            let mut operations = Vec::new();

            for change in &ready_changes {
                if let Some(op) = file_watcher::process_file_change(change) {
                    tracing::info!("Live sync: {:?} -> {:?}", change.kind, change.path);
                    operations.push(op);
                }
            }

            if !operations.is_empty() {
                // Find project dir from first change
                let project_dir = ready_changes.first().map(|c| c.project_dir.clone());

                // Queue batch sync request to plugin
                let request_id = Uuid::new_v4();
                let plugin_request = PluginRequest {
                    id: request_id,
                    command: "sync:batch".to_string(),
                    payload: serde_json::json!({
                        "operations": operations,
                        "source": "file_watcher"  // Mark as from file watcher
                    }),
                };

                // Send to project-specific queue if we know the project
                // Only fall back to global queue if project queue doesn't exist
                let mut sent = false;
                if let Some(ref dir) = project_dir {
                    let mut queues = state.project_queues.write().await;
                    if let Some(queue) = queues.get_mut(dir) {
                        tracing::info!("Queued {} operations for project {}", operations.len(), dir);
                        queue.push_back(plugin_request.clone());
                        sent = true;
                    } else {
                        tracing::warn!("No queue for project {}, available queues: {:?}", dir, queues.keys().collect::<Vec<_>>());
                    }
                }

                // Only use global queue as fallback if project queue wasn't available
                if !sent {
                    let mut queue = state.request_queue.lock().await;
                    queue.push_back(plugin_request);
                }

                // Trigger long-polling requests to wake up
                let _ = state.trigger.send(());
            }
        }

        // Sleep before next check
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
