//! RbxSync Server
//!
//! HTTP server that communicates with the Roblox Studio plugin
//! for game extraction and synchronization.

pub mod git;
pub mod file_watcher;
pub mod harness;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, watch, Mutex, RwLock};
use uuid::Uuid;

/// Normalize Windows paths by converting backslashes to forward slashes.
/// This ensures consistent path handling across platforms and prevents issues
/// with backslash escape sequences in JSON/strings.
/// Windows accepts both forward and backslashes as path separators.
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

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

/// Recursively copy a directory
fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
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

    /// Bot command queue for AI-controlled playtesting
    pub bot_command_queue: Mutex<VecDeque<serde_json::Value>>,

    /// Latest bot state reported from the running game
    pub bot_state: RwLock<Option<serde_json::Value>>,

    /// Bot command results (command_id -> result)
    pub bot_command_results: RwLock<HashMap<Uuid, serde_json::Value>>,

    /// Whether a playtest is currently active (detected via bot heartbeats)
    pub playtest_active: std::sync::atomic::AtomicBool,

    /// Last bot heartbeat timestamp
    pub last_bot_heartbeat: RwLock<Option<std::time::Instant>>,

    /// When playtest was explicitly started (via hello event)
    pub playtest_started: RwLock<Option<std::time::Instant>>,

    /// When playtest was explicitly ended (via goodbye event)
    pub playtest_ended: RwLock<Option<std::time::Instant>>,
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
            bot_command_queue: Mutex::new(VecDeque::new()),
            bot_state: RwLock::new(None),
            bot_command_results: RwLock::new(HashMap::new()),
            playtest_active: std::sync::atomic::AtomicBool::new(false),
            last_bot_heartbeat: RwLock::new(None),
            playtest_started: RwLock::new(None),
            playtest_ended: RwLock::new(None),
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
    /// Whether finalize has been called (extraction complete even if 0 chunks)
    pub finalized: bool,
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
        .route("/rbxsync/check-status", post(handle_check_status))
        .route("/rbxsync/undo-extract", post(handle_undo_extract))
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
        // Bot controller endpoints (AI-powered automated gameplay testing)
        .route("/bot/command", post(handle_bot_command))
        .route("/bot/state", get(handle_bot_state).post(handle_bot_state_update))
        .route("/bot/move", post(handle_bot_move))
        .route("/bot/action", post(handle_bot_action))
        .route("/bot/observe", post(handle_bot_observe))
        // Direct bot command queue (for HTTP polling from running game)
        .route("/bot/queue", post(handle_bot_queue))
        .route("/bot/pending", get(handle_bot_pending))
        .route("/bot/result", post(handle_bot_result_post))
        .route("/bot/result/:id", get(handle_bot_result_get))
        .route("/bot/playtest", get(handle_bot_playtest_status))
        .route("/bot/lifecycle", post(handle_bot_lifecycle))
        // Console output streaming (for E2E testing mode)
        .route("/console/push", post(handle_console_push))
        .route("/console/subscribe", get(handle_console_subscribe))
        .route("/console/history", get(handle_console_history))
        // Run arbitrary Luau code (for MCP)
        .route("/run", post(handle_run_code))
        // Read instance properties (for MCP)
        .route("/read-properties", post(handle_read_properties))
        // Explore game hierarchy (for MCP)
        .route("/explore-hierarchy", post(handle_explore_hierarchy))
        // Find instances by criteria (for MCP)
        .route("/find-instances", post(handle_find_instances))
        // Health check
        .route("/health", get(handle_health))
        // Shutdown endpoint
        .route("/shutdown", post(handle_shutdown))
        // Harness system for multi-session AI development
        .route("/harness/init", post(harness::handle_harness_init))
        .route("/harness/session/start", post(harness::handle_session_start))
        .route("/harness/session/end", post(harness::handle_session_end))
        .route("/harness/feature/update", post(harness::handle_feature_update))
        .route("/harness/status", post(harness::handle_harness_status))
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
    // Normalize path separators for Windows compatibility
    let project_dir = normalize_path(&req.project_dir);

    let mut registry = state.place_registry.write().await;

    // Use session_id as unique key if provided (handles multiple unpublished places with PlaceId=0)
    // Fall back to place_id for backwards compatibility with older plugins
    let key = req.session_id.clone().unwrap_or_else(|| req.place_id.to_string());

    // For published places (place_id > 0), remove any stale entries with the same place_id
    // but a different session_id. This prevents duplicates when Studio is closed and reopened.
    if req.place_id > 0 {
        let stale_keys: Vec<String> = registry
            .iter()
            .filter(|(k, info)| {
                info.place_id == req.place_id && *k != &key
            })
            .map(|(k, _)| k.clone())
            .collect();

        for stale_key in stale_keys {
            if let Some(info) = registry.remove(&stale_key) {
                tracing::info!(
                    "Removed stale session for place {}: {} (old key: {})",
                    req.place_name,
                    info.session_id.unwrap_or_default(),
                    stale_key
                );
            }
        }
    }

    // Register/update this place (replaces any existing entry for this session)
    registry.insert(key.clone(), PlaceInfo {
        place_id: req.place_id,
        place_name: req.place_name.clone(),
        project_dir: project_dir.clone(),
        session_id: req.session_id.clone(),
        last_heartbeat: Some(Instant::now()),
    });
    drop(registry); // Release lock before acquiring another

    // Create project queue if it doesn't exist
    {
        let mut queues = state.project_queues.write().await;
        queues.entry(project_dir.clone()).or_insert_with(VecDeque::new);
    }

    // Only log once per session to prevent spam
    let mut logged = state.logged_studio_places.write().await;
    if !logged.contains(&key) {
        logged.insert(key.clone());
        tracing::info!(
            "Studio registered: {} (PlaceId: {}, SessionId: {:?}, Key: {}) -> {}",
            req.place_name,
            req.place_id,
            req.session_id,
            key,
            project_dir
        );

        // Check for path mismatch with VS Code workspaces
        let workspaces = state.vscode_workspaces.read().await;
        if !workspaces.is_empty() {
            let studio_dir = project_dir.as_str();
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

    // Normalize path separators for Windows compatibility
    let workspace_dir = normalize_path(&req.workspace_dir);

    // Update heartbeat timestamp
    let mut workspaces = state.vscode_workspaces.write().await;
    let is_new = !workspaces.contains_key(&workspace_dir);
    workspaces.insert(workspace_dir.clone(), VsCodeWorkspace {
        workspace_dir: workspace_dir.clone(),
        last_heartbeat: Some(Instant::now()),
    });
    drop(workspaces); // Release lock before acquiring another

    // Only log and start file watcher if this is a new workspace this session
    // Use a separate set to prevent spam from heartbeat registrations
    let mut logged = state.logged_vscode_workspaces.write().await;
    let should_log = !logged.contains(&workspace_dir);
    if should_log {
        logged.insert(workspace_dir.clone());
        drop(logged); // Release lock

        tracing::info!("VS Code workspace registered: {}", workspace_dir);

        // Check for path mismatch with Studio registrations
        let registry = state.place_registry.read().await;
        if !registry.is_empty() {
            let studio_dirs: Vec<&str> = registry.values().map(|p| p.project_dir.as_str()).collect();
            let vscode_dir = workspace_dir.as_str();

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
            let dir = workspace_dir.clone();

            // Load config to check package sync settings
            let config = load_project_config(&dir);
            let packages_config = config.as_ref().and_then(|c| c.get("packages"));

            // Check if packages should sync (excludeFromWatch: false means sync packages)
            // Also enable if packages.enabled is true and excludeFromWatch is not explicitly set
            let sync_packages = packages_config
                .and_then(|p| p.get("excludeFromWatch"))
                .and_then(|v| v.as_bool())
                .map(|exclude| !exclude)  // Invert: excludeFromWatch=false means sync_packages=true
                .unwrap_or(false);  // Default: don't sync packages (for backwards compatibility)

            tokio::spawn(async move {
                if let Err(e) = file_watcher::start_file_watcher(dir, watcher_state, sync_packages).await {
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

/// Check link status for a Studio session (used by Studio to detect VS Code unlink)
#[derive(Deserialize)]
struct CheckStatusRequest {
    session_id: String,
}

async fn handle_check_status(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CheckStatusRequest>,
) -> impl IntoResponse {
    let registry = state.place_registry.read().await;

    if let Some(place_info) = registry.get(&req.session_id) {
        Json(serde_json::json!({
            "success": true,
            "linked": !place_info.project_dir.is_empty(),
            "project_dir": place_info.project_dir,
            "place_name": place_info.place_name
        }))
    } else {
        Json(serde_json::json!({
            "success": false,
            "linked": false,
            "error": "Session not found"
        }))
    }
}

/// Undo last extraction by restoring from backup
#[derive(Deserialize)]
struct UndoExtractRequest {
    project_dir: String,
}

async fn handle_undo_extract(
    Json(req): Json<UndoExtractRequest>,
) -> impl IntoResponse {
    let src_dir = PathBuf::from(&req.project_dir).join("src");
    let backup_dir = PathBuf::from(&req.project_dir).join(".rbxsync-backup");
    let backup_src = backup_dir.join("src");

    if !backup_src.exists() {
        return Json(serde_json::json!({
            "success": false,
            "error": "No backup found to restore"
        }));
    }

    // Remove current src
    if src_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&src_dir) {
            return Json(serde_json::json!({
                "success": false,
                "error": format!("Failed to remove current src: {}", e)
            }));
        }
    }

    // Restore from backup
    if let Err(e) = std::fs::rename(&backup_src, &src_dir) {
        // If rename fails, try copy
        if let Err(e) = copy_dir_recursive(&backup_src, &src_dir) {
            return Json(serde_json::json!({
                "success": false,
                "error": format!("Failed to restore from backup: {}", e)
            }));
        }
        let _ = std::fs::remove_dir_all(&backup_src);
    }

    tracing::info!("Restored src from backup for {}", req.project_dir);

    Json(serde_json::json!({
        "success": true,
        "message": "Extraction undone - src restored from backup"
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
            finalized: false,
        });
    }

    // Pause live sync during extraction to avoid syncing back files we just extracted
    state.live_sync_paused.store(true, std::sync::atomic::Ordering::Relaxed);
    tracing::info!("Live sync paused for extraction");

    // Clear any pending sync commands from queues to prevent them from interfering with extraction
    // This is important because sync commands queued before extraction would try to sync
    // files back to Studio before extraction completes
    {
        let mut global_queue = state.request_queue.lock().await;
        let before_count = global_queue.len();
        global_queue.retain(|req| !req.command.starts_with("sync:"));
        let removed = before_count - global_queue.len();
        if removed > 0 {
            tracing::info!("Cleared {} pending sync commands from global queue before extraction", removed);
        }
    }
    {
        let mut project_queues = state.project_queues.write().await;
        for (project_dir, queue) in project_queues.iter_mut() {
            let before_count = queue.len();
            queue.retain(|req| !req.command.starts_with("sync:"));
            let removed = before_count - queue.len();
            if removed > 0 {
                tracing::info!("Cleared {} pending sync commands from queue for {} before extraction", removed, project_dir);
            }
        }
    }

    // Also drain any pending file change events to prevent them from being queued after extraction
    {
        let mut rx = state.file_change_rx.lock().await;
        let mut drained = 0;
        while rx.try_recv().is_ok() {
            drained += 1;
        }
        if drained > 0 {
            tracing::info!("Drained {} pending file change events before extraction", drained);
        }
    }

    // Clear existing src folder before extraction to remove stale files (Fixes RBXSYNC-27)
    if let Some(ref project_dir) = req.project_dir {
        if !project_dir.is_empty() {
            let src_dir = PathBuf::from(project_dir).join("src");

            if src_dir.exists() {
                let backup_dir = PathBuf::from(project_dir).join(".rbxsync-backup");
                let backup_src = backup_dir.join("src");

                // Remove old backup if exists
                if backup_src.exists() {
                    let _ = std::fs::remove_dir_all(&backup_src);
                }

                // Create backup directory
                let _ = std::fs::create_dir_all(&backup_dir);

                // Move src to backup (rename is atomic and fast)
                if let Err(e) = std::fs::rename(&src_dir, &backup_src) {
                    // If rename fails (cross-device), fall back to copy+delete
                    tracing::warn!("Rename failed, falling back to copy: {}", e);
                    if let Err(e) = copy_dir_recursive(&src_dir, &backup_src) {
                        tracing::warn!("Failed to backup src directory: {}", e);
                    }
                    // Delete original src after backup
                    let _ = std::fs::remove_dir_all(&src_dir);
                }

                tracing::info!("Cleared src folder before extraction (backed up to .rbxsync-backup/src)");
            }

            // Create fresh src directory
            let _ = std::fs::create_dir_all(&src_dir);
        }
    }

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
            finalized: false,
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
        // Complete if finalized (handles 0 chunks case) OR all chunks received
        let complete = s.finalized || s.total_chunks.map(|t| s.chunks_received >= t).unwrap_or(false);
        Json(serde_json::json!({
            "sessionId": s.id,
            "chunksReceived": s.chunks_received,
            "totalChunks": s.total_chunks,
            "complete": complete,
            "finalized": s.finalized
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
    let session_guard = state.extraction_session.read().await;

    if session_guard.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "No extraction session active"
            })),
        );
    }

    let session = session_guard.as_ref().unwrap();
    let src_dir = PathBuf::from(&req.project_dir).join("src");

    // Load project config and tree mapping
    let config = load_project_config(&req.project_dir);
    let tree_mapping = get_tree_mapping(&config);
    tracing::info!("Tree mapping loaded: {:?}", tree_mapping);

    // Check package preservation settings from config JSON
    let (preserve_packages, packages_folder) = if let Some(ref cfg) = config {
        let packages_config = cfg.get("packages");
        let enabled = packages_config
            .and_then(|p| p.get("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true); // Default to true if packages section exists
        let preserve = packages_config
            .and_then(|p| p.get("preserveOnExtract"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true); // Default to true
        let folder = packages_config
            .and_then(|p| p.get("packagesFolder"))
            .and_then(|v| v.as_str())
            .unwrap_or("Packages")
            .to_string();
        (packages_config.is_some() && enabled && preserve, folder)
    } else {
        (false, "Packages".to_string())
    };
    if preserve_packages {
        tracing::info!("Package preservation enabled - Packages folder: {}", packages_folder);
    }

    // Backup existing src directory before clearing (for undo support)
    let backup_dir = PathBuf::from(&req.project_dir).join(".rbxsync-backup");
    let backup_src = backup_dir.join("src");

    // IMPORTANT: Back up terrain data BEFORE any directory operations
    // Terrain is saved during extraction and must survive the src clear
    let terrain_file = src_dir.join("Workspace").join("Terrain").join("terrain.rbxjson");
    let terrain_data = if terrain_file.exists() {
        tracing::info!("Backing up terrain.rbxjson before finalize");
        std::fs::read_to_string(&terrain_file).ok()
    } else {
        None
    };

    if src_dir.exists() {
        // Remove old backup if exists
        if backup_src.exists() {
            let _ = std::fs::remove_dir_all(&backup_src);
        }
        // Create backup directory
        let _ = std::fs::create_dir_all(&backup_dir);
        // Move src to backup (rename is atomic and fast)
        if let Err(e) = std::fs::rename(&src_dir, &backup_src) {
            // If rename fails (cross-device), fall back to copy+delete
            tracing::warn!("Rename failed, falling back to copy: {}", e);
            if let Err(e) = copy_dir_recursive(&src_dir, &backup_src) {
                tracing::warn!("Failed to backup src directory: {}", e);
            }

            for entry in std::fs::read_dir(&src_dir).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()) {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        let _ = std::fs::remove_dir_all(&path);
                    } else {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
        tracing::info!("Backed up src to .rbxsync-backup/src");
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

    // Restore terrain data that was backed up before clearing
    if let Some(data) = terrain_data {
        let terrain_dir = src_dir.join("Workspace").join("Terrain");
        let _ = std::fs::create_dir_all(&terrain_dir);
        if std::fs::write(terrain_dir.join("terrain.rbxjson"), &data).is_ok() {
            tracing::info!("Restored terrain.rbxjson after finalize");
        }
    }

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

    // PERFORMANCE OPTIMIZATION for large games (RBXSYNC-26):
    // Instead of writing files sequentially (which causes PC hang on 180k+ instances),
    // we batch directory creation and write files in parallel with bounded concurrency.

    use futures::stream::{self, StreamExt};

    // Maximum concurrent file writes to prevent overwhelming the filesystem
    const MAX_CONCURRENT_WRITES: usize = 64;

    // Struct to hold pending write operations
    struct WriteOp {
        path: PathBuf,
        content: String,
    }

    // First pass: Collect all directories needed and prepare write operations
    let mut directories_needed: HashSet<PathBuf> = HashSet::new();
    let mut script_write_ops: Vec<WriteOp> = Vec::new();
    let mut json_write_ops: Vec<WriteOp> = Vec::new();

    tracing::info!("Preparing {} instances for parallel write...", all_instances.len());
    let prep_start = std::time::Instant::now();

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

        // Collect parent directory instead of creating immediately
        if let Some(parent) = full_path.parent() {
            directories_needed.insert(parent.to_path_buf());
        }

        // Check if this instance has children (use normalized path)
        let is_container = has_children(&inst_path);

        // Check if this is a script with source
        let is_script = matches!(class_name, "Script" | "LocalScript" | "ModuleScript");

        if is_script {
            // Prepare script source write operation
            if let Some(props) = inst.get("properties") {
                if let Some(source) = props.get("Source").and_then(|v| v.get("value")).and_then(|v| v.as_str()) {
                    let extension = match class_name {
                        "Script" => ".server.luau",
                        "LocalScript" => ".client.luau",
                        _ => ".luau",
                    };
                    let script_path = rbxsync_core::path_with_suffix(&full_path, extension);
                    script_write_ops.push(WriteOp {
                        path: PathBuf::from(script_path),
                        content: source.to_string(),
                    });
                }
            }
        }

        // Prepare .rbxjson file write operation
        let json_path = if is_container {
            // Container: folder will be created, put _meta.rbxjson inside
            directories_needed.insert(full_path.clone());
            full_path.join("_meta.rbxjson")
        } else {
            // Leaf: write as sibling .rbxjson
            rbxsync_core::pathbuf_with_suffix(&full_path, ".rbxjson")
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
            json_write_ops.push(WriteOp {
                path: json_path,
                content: json,
            });
        }
    }

    tracing::info!(
        "Preparation complete in {:?}: {} directories, {} scripts, {} json files",
        prep_start.elapsed(),
        directories_needed.len(),
        script_write_ops.len(),
        json_write_ops.len()
    );

    // Batch create all directories (run in blocking task to not block async runtime)
    let dirs_to_create: Vec<PathBuf> = directories_needed.into_iter().collect();
    let dir_count = dirs_to_create.len();

    let dir_start = std::time::Instant::now();
    tokio::task::spawn_blocking(move || {
        for dir in dirs_to_create {
            let _ = std::fs::create_dir_all(&dir);
        }
    }).await.unwrap_or_else(|e| {
        tracing::error!("Failed to create directories: {}", e);
    });
    tracing::info!("Created {} directories in {:?}", dir_count, dir_start.elapsed());

    // Write files in parallel with bounded concurrency
    let write_start = std::time::Instant::now();

    // Write scripts in parallel
    let script_count = script_write_ops.len();
    let script_results: Vec<bool> = stream::iter(script_write_ops)
        .map(|op| async move {
            tokio::fs::write(&op.path, &op.content).await.is_ok()
        })
        .buffer_unordered(MAX_CONCURRENT_WRITES)
        .collect()
        .await;
    let scripts_written = script_results.iter().filter(|&&ok| ok).count();

    // Write JSON files in parallel
    let json_count = json_write_ops.len();
    let json_results: Vec<bool> = stream::iter(json_write_ops)
        .map(|op| async move {
            tokio::fs::write(&op.path, &op.content).await.is_ok()
        })
        .buffer_unordered(MAX_CONCURRENT_WRITES)
        .collect()
        .await;
    let files_written = json_results.iter().filter(|&&ok| ok).count();

    tracing::info!(
        "Wrote {} scripts and {} json files in {:?} ({} concurrent writes)",
        scripts_written, files_written, write_start.elapsed(), MAX_CONCURRENT_WRITES
    );

    // Log if there were any failures
    let script_failures = script_count - scripts_written;
    let json_failures = json_count - files_written;
    if script_failures > 0 || json_failures > 0 {
        tracing::warn!(
            "Write failures: {} scripts, {} json files",
            script_failures, json_failures
        );
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

    // Restore Packages folder from backup if preservation is enabled
    let mut packages_preserved = false;
    if preserve_packages {
        // Look for Packages folders in common locations within backup
        let package_restore_locations: Vec<(String, String)> = vec![
            ("ReplicatedStorage/Packages".to_string(), "ReplicatedStorage/Packages".to_string()),
            ("ServerScriptService/Packages".to_string(), "ServerScriptService/Packages".to_string()),
            ("ServerStorage/Packages".to_string(), "ServerStorage/Packages".to_string()),
            // Also check root-level Packages folder
            (packages_folder.clone(), packages_folder.clone()),
        ];

        for (backup_rel, dest_rel) in &package_restore_locations {
            let backup_packages = backup_src.join(backup_rel);
            let dest_packages = src_dir.join(dest_rel);

            if backup_packages.exists() && backup_packages.is_dir() {
                // Remove any extracted packages (from Studio) to replace with local
                if dest_packages.exists() {
                    let _ = std::fs::remove_dir_all(&dest_packages);
                }

                // Ensure parent directory exists
                if let Some(parent) = dest_packages.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                // Restore packages from backup
                if let Err(e) = copy_dir_recursive(&backup_packages, &dest_packages) {
                    tracing::warn!("Failed to restore packages from {}: {}", backup_rel, e);
                } else {
                    tracing::info!("Restored Wally packages from backup: {}", backup_rel);
                    packages_preserved = true;
                }
            }
        }
    }

    tracing::info!(
        "Finalize complete: {} .rbxjson files, {} .luau scripts, {} services{}",
        files_written,
        scripts_written,
        service_folders.len(),
        if packages_preserved { ", packages preserved" } else { "" }
    );

    // Clear any file change events that accumulated during extraction (from the files we just wrote)
    // This prevents them from being synced back to Studio after extraction
    // We do this in a spawned task to avoid blocking the response
    let state_for_cleanup = state.clone();
    tokio::spawn(async move {
        // Wait for file system events to settle
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Drain any accumulated file changes
        {
            let mut rx = state_for_cleanup.file_change_rx.lock().await;
            let mut drained = 0;
            while rx.try_recv().is_ok() {
                drained += 1;
            }
            if drained > 0 {
                tracing::info!("Drained {} file change events generated during extraction", drained);
            }
        }

        // Resume live sync after cleanup
        state_for_cleanup.live_sync_paused.store(false, std::sync::atomic::Ordering::Relaxed);
        tracing::info!("Live sync resumed after extraction");
    });

    // Mark session as finalized so status endpoint returns complete=true
    // This is important when there are 0 chunks (excluded services case)
    // Drop the read lock so we can take a write lock
    drop(session_guard);
    {
        let mut session_write = state.extraction_session.write().await;
        if let Some(ref mut s) = *session_write {
            s.finalized = true;
            tracing::info!("Extraction session marked as finalized");
        }
    }

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
    tracing::info!("Received terrain data for project: {}", req.project_dir);
    let terrain_dir = PathBuf::from(&req.project_dir).join("src").join("Workspace").join("Terrain");
    tracing::info!("Terrain directory: {}", terrain_dir.display());

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
    pub change_type: String,  // "create", "modify", "delete", "rename"
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
                    let script_path = rbxsync_core::path_with_suffix(&full_path, ext);
                    if std::fs::remove_file(&script_path).is_ok() {
                        deleted_any = true;
                        tracing::info!("Studio sync: deleted {}", script_path);
                    }
                }
                let json_path = rbxsync_core::path_with_suffix(&full_path, ".rbxjson");
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
            "rename" => {
                // Handle rename: move files from old path to new path
                if let Some(data) = &op.data {
                    let old_inst_path = data.get("oldPath").and_then(|v| v.as_str());
                    let new_inst_path = data.get("newPath").and_then(|v| v.as_str());

                    if let (Some(old_path), Some(new_path)) = (old_inst_path, new_inst_path) {
                        let old_fs_path = apply_tree_mapping(old_path, &tree_mapping);
                        let new_fs_path = apply_tree_mapping(new_path, &tree_mapping);
                        let old_full_path = src_dir.join(&old_fs_path);
                        let new_full_path = src_dir.join(&new_fs_path);

                        tracing::info!("Studio sync: renaming {:?} -> {:?}", old_full_path, new_full_path);

                        // Ensure new parent directory exists
                        if let Some(parent) = new_full_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }

                        // Try to rename directory (for folders with children)
                        if old_full_path.is_dir() {
                            match std::fs::rename(&old_full_path, &new_full_path) {
                                Ok(_) => {
                                    tracing::info!("Studio sync: renamed folder {:?} -> {:?}", old_full_path, new_full_path);
                                    files_written += 1;
                                }
                                Err(e) => {
                                    errors.push(format!("Failed to rename folder {:?}: {}", old_full_path, e));
                                }
                            }
                        } else {
                            // Rename script files (try all extensions)
                            let extensions = [".server.luau", ".client.luau", ".luau", ".rbxjson"];
                            let mut renamed_any = false;
                            for ext in extensions {
                                let old_file_str = rbxsync_core::path_with_suffix(&old_full_path, ext);
                                let new_file_str = rbxsync_core::path_with_suffix(&new_full_path, ext);
                                let old_file = PathBuf::from(&old_file_str);
                                let new_file = PathBuf::from(&new_file_str);
                                if old_file.exists() {
                                    match std::fs::rename(&old_file, &new_file) {
                                        Ok(_) => {
                                            tracing::info!("Studio sync: renamed {:?} -> {:?}", old_file, new_file);
                                            renamed_any = true;
                                        }
                                        Err(e) => {
                                            errors.push(format!("Failed to rename {:?}: {}", old_file, e));
                                        }
                                    }
                                }
                            }
                            if renamed_any {
                                files_written += 1;
                            }
                        }
                    } else {
                        errors.push("Rename operation missing oldPath or newPath".to_string());
                    }
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
                            let script_path = rbxsync_core::path_with_suffix(&full_path, extension);

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

                    let json_path = rbxsync_core::path_with_suffix(&full_path, ".rbxjson");
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
    let project_dir = PathBuf::from(&req.project_dir);
    let src_dir = project_dir.join("src");

    if !src_dir.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "Source directory does not exist"
            })),
        );
    }

    // Load project config for package settings
    let config = load_project_config(&req.project_dir);
    let packages_config = config.as_ref().and_then(|c| c.get("packages"));
    let packages_folder = packages_config
        .and_then(|p| p.get("packagesFolder"))
        .and_then(|v| v.as_str())
        .unwrap_or("Packages");

    // Auto-detect packages: enabled if explicitly set, OR if Packages folder exists (zero-config)
    let packages_dir = project_dir.join(packages_folder);
    let packages_enabled = packages_config
        .and_then(|p| p.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| packages_dir.exists() && packages_dir.is_dir());
    let shared_packages_path = packages_config
        .and_then(|p| p.get("sharedPackagesPath"))
        .and_then(|v| v.as_str())
        .unwrap_or("ReplicatedStorage/Packages");
    let server_packages_path = packages_config
        .and_then(|p| p.get("serverPackagesPath"))
        .and_then(|v| v.as_str())
        .unwrap_or("ServerScriptService/Packages");

    // Recursively read all .rbxjson files
    let mut instances: Vec<serde_json::Value> = Vec::new();
    let mut scripts: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    fn walk_dir(
        dir: &std::path::Path,
        base: &std::path::Path,
        path_prefix: &str,
        instances: &mut Vec<serde_json::Value>,
        scripts: &mut std::collections::HashMap<String, String>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_dir(&path, base, path_prefix, instances, scripts);
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
                                let path_str = rbxsync_core::path_to_string(rel_path);
                                // Convert file path to instance path:
                                // e.g., "Workspace/MyPart.rbxjson" -> "Workspace/MyPart"
                                // e.g., "Workspace/MyPart/_meta.rbxjson" -> "Workspace/MyPart"
                                let is_meta = path_str.ends_with("/_meta.rbxjson") || path_str.ends_with("\\_meta.rbxjson");
                                let rel_inst_path = if is_meta {
                                    // _meta.rbxjson represents the parent folder
                                    path_str.replace("/_meta.rbxjson", "").replace("\\_meta.rbxjson", "")
                                } else {
                                    path_str.replace(".rbxjson", "")
                                };

                                // Apply path prefix (for packages mapping to DataModel paths)
                                let inst_path = if path_prefix.is_empty() {
                                    rel_inst_path
                                } else {
                                    format!("{}/{}", path_prefix, rel_inst_path)
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
                        let path_str = rbxsync_core::path_to_string(rel_path);
                        // Keep '/' as delimiter (matches instance path format)
                        // e.g., "ServerScriptService/MyScript.server.luau" -> "ServerScriptService/MyScript"
                        let rel_inst_path = path_str
                            .trim_end_matches(".server.luau")
                            .trim_end_matches(".client.luau")
                            .trim_end_matches(".luau")
                            .to_string();

                        // Apply path prefix (for packages mapping to DataModel paths)
                        let inst_path = if path_prefix.is_empty() {
                            rel_inst_path
                        } else {
                            format!("{}/{}", path_prefix, rel_inst_path)
                        };

                        if let Ok(source) = std::fs::read_to_string(&path) {
                            scripts.insert(inst_path, source);
                        }
                    }
                }
            }
        }
    }

    // Walk the main src directory (no prefix - paths map directly to DataModel)
    walk_dir(&src_dir, &src_dir, "", &mut instances, &mut scripts);

    // Walk packages directory if enabled (packages_dir already validated when packages_enabled was set)
    if packages_enabled {
        tracing::info!("Reading Wally packages from {} -> {}", packages_folder, shared_packages_path);
        walk_dir(&packages_dir, &packages_dir, shared_packages_path, &mut instances, &mut scripts);

        // Also check for server packages subdirectory
        let server_pkg_dir = packages_dir.join("ServerPackages");
        if server_pkg_dir.exists() && server_pkg_dir.is_dir() {
            tracing::info!("Reading server packages from ServerPackages -> {}", server_packages_path);
            walk_dir(&server_pkg_dir, &server_pkg_dir, server_packages_path, &mut instances, &mut scripts);
        }
    }

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
    // Try both possible terrain file locations
    let terrain_file_v1 = PathBuf::from(&req.project_dir)
        .join("src")
        .join("Workspace")
        .join("Terrain.rbxjson");
    let terrain_file_v2 = PathBuf::from(&req.project_dir)
        .join("src")
        .join("Workspace")
        .join("Terrain")
        .join("terrain.rbxjson");

    let terrain_file = if terrain_file_v1.exists() {
        terrain_file_v1
    } else {
        terrain_file_v2
    };

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
                                let path_str = rbxsync_core::path_to_string(rel_path);
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
                        let path_str = rbxsync_core::path_to_string(rel_path);
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
                                let path_str = rbxsync_core::path_to_string(rel_path);
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
// Bot Controller Endpoints (AI-powered automated gameplay testing)
// ============================================================================

/// Generic bot command request
#[derive(Debug, Deserialize)]
struct BotCommandRequest {
    #[serde(rename = "type")]
    command_type: String,
    command: String,
    #[serde(default)]
    args: serde_json::Value,
}

/// Bot movement request
#[derive(Debug, Deserialize)]
struct BotMoveRequest {
    #[serde(default)]
    position: Option<serde_json::Value>,
    #[serde(default)]
    object: Option<String>,
    #[serde(rename = "objectName", default)]
    object_name: Option<String>,
}

/// Bot action request
#[derive(Debug, Deserialize)]
struct BotActionRequest {
    action: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "toolName", default)]
    tool_name: Option<String>,
    #[serde(rename = "objectName", default)]
    object_name: Option<String>,
}

/// Bot observe request
#[derive(Debug, Deserialize)]
struct BotObserveRequest {
    #[serde(rename = "type", default = "default_observe_type")]
    observe_type: String,
    #[serde(default)]
    radius: Option<f64>,
    #[serde(default)]
    query: Option<String>,
}

fn default_observe_type() -> String {
    "state".to_string()
}

/// Helper function to send a bot command to the plugin
async fn send_bot_command(
    state: &Arc<AppState>,
    command: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, (StatusCode, Json<serde_json::Value>)> {
    let request_id = Uuid::new_v4();
    let request = PluginRequest {
        id: request_id,
        command: command.to_string(),
        payload,
    };

    // Create response channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.response_channels.write().await.insert(request_id, tx);

    // Queue the request
    state.request_queue.lock().await.push_back(request);
    state.trigger.send(()).ok();

    // Wait for response with timeout (longer timeout for movement commands)
    let timeout = if command == "bot:move" {
        tokio::time::Duration::from_secs(60)
    } else {
        tokio::time::Duration::from_secs(30)
    };

    match tokio::time::timeout(timeout, rx.recv()).await {
        Ok(Some(response)) => {
            state.response_channels.write().await.remove(&request_id);
            Ok(response.data)
        }
        Ok(None) => {
            state.response_channels.write().await.remove(&request_id);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "Channel closed"
                })),
            ))
        }
        Err(_) => {
            state.response_channels.write().await.remove(&request_id);
            Err((
                StatusCode::REQUEST_TIMEOUT,
                Json(serde_json::json!({
                    "success": false,
                    "error": "Bot command timeout - ensure playtest is running"
                })),
            ))
        }
    }
}

/// Handle generic bot command
async fn handle_bot_command(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BotCommandRequest>,
) -> impl IntoResponse {
    let payload = serde_json::json!({
        "type": req.command_type,
        "command": req.command,
        "args": req.args
    });

    match send_bot_command(&state, "bot:command", payload).await {
        Ok(data) => (StatusCode::OK, Json(data)),
        Err((status, json)) => (status, json),
    }
}

/// Handle bot state observation (GET)
async fn handle_bot_state(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match send_bot_command(&state, "bot:state", serde_json::json!({})).await {
        Ok(data) => (StatusCode::OK, Json(data)),
        Err((status, json)) => (status, json),
    }
}

/// Handle bot movement command
async fn handle_bot_move(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BotMoveRequest>,
) -> impl IntoResponse {
    let payload = serde_json::json!({
        "position": req.position,
        "object": req.object,
        "objectName": req.object_name
    });

    match send_bot_command(&state, "bot:move", payload).await {
        Ok(data) => (StatusCode::OK, Json(data)),
        Err((status, json)) => (status, json),
    }
}

/// Handle bot action command
async fn handle_bot_action(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BotActionRequest>,
) -> impl IntoResponse {
    let payload = serde_json::json!({
        "action": req.action,
        "name": req.name.or(req.tool_name),
        "objectName": req.object_name
    });

    match send_bot_command(&state, "bot:action", payload).await {
        Ok(data) => (StatusCode::OK, Json(data)),
        Err((status, json)) => (status, json),
    }
}

/// Handle bot observation command
async fn handle_bot_observe(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BotObserveRequest>,
) -> impl IntoResponse {
    let payload = serde_json::json!({
        "type": req.observe_type,
        "radius": req.radius,
        "query": req.query
    });

    match send_bot_command(&state, "bot:observe", payload).await {
        Ok(data) => (StatusCode::OK, Json(data)),
        Err((status, json)) => (status, json),
    }
}

/// Receive state update from running game (POST /bot/state)
async fn handle_bot_state_update(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Update bot state
    let mut bot_state = state.bot_state.write().await;
    *bot_state = Some(body);

    // Mark playtest as active and update heartbeat
    state.playtest_active.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut heartbeat = state.last_bot_heartbeat.write().await;
    *heartbeat = Some(std::time::Instant::now());

    Json(serde_json::json!({ "success": true }))
}

/// Queue a command for the bot to execute (POST /bot/queue)
async fn handle_bot_queue(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();

    // Wrap the command with an ID
    let cmd_with_id = serde_json::json!({
        "id": id.to_string(),
        "command": body
    });

    let mut queue = state.bot_command_queue.lock().await;
    queue.push_back(cmd_with_id);

    Json(serde_json::json!({
        "success": true,
        "queued": true,
        "id": id.to_string(),
        "queue_length": queue.len()
    }))
}

/// Get next pending command for bot (GET /bot/pending)
async fn handle_bot_pending(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut queue = state.bot_command_queue.lock().await;
    if let Some(cmd) = queue.pop_front() {
        Json(serde_json::json!({
            "success": true,
            "command": cmd.get("command").cloned(),
            "id": cmd.get("id").and_then(|v| v.as_str())
        }))
    } else {
        Json(serde_json::json!({
            "success": true,
            "command": null
        }))
    }
}

/// Receive command result from bot (POST /bot/result)
async fn handle_bot_result_post(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Extract command ID from result
    let id_str = body.get("id").and_then(|v| v.as_str());

    if let Some(id_str) = id_str {
        if let Ok(id) = Uuid::parse_str(id_str) {
            // Store the result
            let mut results = state.bot_command_results.write().await;
            results.insert(id, body.clone());

            // Also update playtest heartbeat
            state.playtest_active.store(true, std::sync::atomic::Ordering::Relaxed);
            let mut heartbeat = state.last_bot_heartbeat.write().await;
            *heartbeat = Some(std::time::Instant::now());

            return Json(serde_json::json!({
                "success": true,
                "stored": true,
                "id": id_str
            }));
        }
    }

    Json(serde_json::json!({
        "success": false,
        "error": "Missing or invalid command ID"
    }))
}

/// Get command result by ID (GET /bot/result/:id)
async fn handle_bot_result_get(
    State(state): State<Arc<AppState>>,
    Path(id_str): Path<String>,
) -> impl IntoResponse {
    if let Ok(id) = Uuid::parse_str(&id_str) {
        let results = state.bot_command_results.read().await;
        if let Some(result) = results.get(&id) {
            return Json(serde_json::json!({
                "success": true,
                "found": true,
                "result": result
            }));
        }
    }

    Json(serde_json::json!({
        "success": true,
        "found": false,
        "result": null
    }))
}

/// Check if playtest is active (GET /bot/playtest)
async fn handle_bot_playtest_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let is_active = state.playtest_active.load(std::sync::atomic::Ordering::Relaxed);

    // Check if heartbeat is stale (> 2 seconds - reduced from 5 for faster detection)
    let heartbeat = state.last_bot_heartbeat.read().await;
    let stale = if let Some(last) = *heartbeat {
        last.elapsed().as_secs_f64() > 2.0
    } else {
        true
    };

    // Mark as inactive if stale
    if stale && is_active {
        state.playtest_active.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    // Check explicit lifecycle events
    let started = state.playtest_started.read().await;
    let ended = state.playtest_ended.read().await;
    let explicitly_ended = ended.is_some();

    let current_state = state.bot_state.read().await;

    Json(serde_json::json!({
        "success": true,
        "active": is_active && !stale,
        "stale": stale,
        "explicitly_ended": explicitly_ended,
        "last_heartbeat_ago": heartbeat.map(|h| h.elapsed().as_secs_f64()),
        "started_at": started.map(|s| s.elapsed().as_secs_f64()),
        "ended_at": ended.map(|e| e.elapsed().as_secs_f64()),
        "last_state": *current_state
    }))
}

/// Handle bot lifecycle events (hello/goodbye) (POST /bot/lifecycle)
async fn handle_bot_lifecycle(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let event = body.get("event").and_then(|v| v.as_str()).unwrap_or("");
    let reason = body.get("reason").and_then(|v| v.as_str());

    match event {
        "hello" => {
            // Bot connected - playtest started
            state.playtest_active.store(true, std::sync::atomic::Ordering::Relaxed);
            *state.playtest_started.write().await = Some(std::time::Instant::now());
            *state.playtest_ended.write().await = None;
            *state.last_bot_heartbeat.write().await = Some(std::time::Instant::now());
            tracing::info!("Playtest started - bot connected");

            Json(serde_json::json!({
                "success": true,
                "event": "hello",
                "message": "Bot registered"
            }))
        }
        "goodbye" => {
            // Bot disconnected - playtest ended
            state.playtest_active.store(false, std::sync::atomic::Ordering::Relaxed);
            *state.playtest_ended.write().await = Some(std::time::Instant::now());
            tracing::info!("Playtest ended - bot disconnected (reason: {:?})", reason);

            Json(serde_json::json!({
                "success": true,
                "event": "goodbye",
                "message": "Bot unregistered"
            }))
        }
        _ => {
            Json(serde_json::json!({
                "success": false,
                "error": format!("Unknown lifecycle event: {}", event)
            }))
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

// ============================================================================
// Read Properties Endpoint
// ============================================================================

/// Request structure for reading instance properties
#[derive(Debug, Deserialize)]
struct ReadPropertiesRequest {
    path: String,
}

/// Read properties of an instance at the given path (for MCP integration)
async fn handle_read_properties(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReadPropertiesRequest>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4();
    tracing::info!("read-properties:get request {} - path: {}", request_id, req.path);
    let request = PluginRequest {
        id: request_id,
        command: "read-properties:get".to_string(),
        payload: serde_json::json!({
            "path": req.path
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
    tracing::info!("read-properties:get request {} - queued (queue length: {})", request_id, queue_len);
    state.trigger.send(()).ok();

    // Wait for response with timeout
    let timeout = tokio::time::Duration::from_secs(30);
    match tokio::time::timeout(timeout, rx.recv()).await {
        Ok(Some(response)) => {
            state.response_channels.write().await.remove(&request_id);
            (StatusCode::OK, Json(serde_json::json!({
                "success": response.success,
                "data": response.data,
                "error": response.error
            })))
        }
        Ok(None) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "data": null,
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
                    "data": null,
                    "error": "Plugin response timeout"
                })),
            )
        }
    }
}

// ============================================================================
// Explore Hierarchy Endpoint
// ============================================================================

/// Request structure for exploring game hierarchy
#[derive(Debug, Deserialize)]
struct ExploreHierarchyRequest {
    path: Option<String>,
    depth: Option<u32>,
}

/// Explore the game hierarchy (for MCP integration)
async fn handle_explore_hierarchy(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExploreHierarchyRequest>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4();
    let depth = req.depth.unwrap_or(1).min(10);
    tracing::info!(
        "explore-hierarchy:get request {} - path: {:?}, depth: {}",
        request_id,
        req.path,
        depth
    );
    let request = PluginRequest {
        id: request_id,
        command: "explore-hierarchy:get".to_string(),
        payload: serde_json::json!({
            "path": req.path,
            "depth": depth
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
    tracing::info!(
        "explore-hierarchy:get request {} - queued (queue length: {})",
        request_id,
        queue_len
    );
    state.trigger.send(()).ok();

    // Wait for response with timeout (longer for deep hierarchies)
    let timeout = tokio::time::Duration::from_secs(60);
    match tokio::time::timeout(timeout, rx.recv()).await {
        Ok(Some(response)) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": response.success,
                    "data": response.data,
                    "error": response.error
                })),
            )
        }
        Ok(None) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "data": null,
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
                    "data": null,
                    "error": "Plugin response timeout"
                })),
            )
        }
    }
}

// ============================================================================
// Find Instances Endpoint
// ============================================================================

/// Request structure for finding instances
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindInstancesRequest {
    class_name: Option<String>,
    name: Option<String>,
    parent: Option<String>,
    limit: Option<u32>,
}

/// Find instances matching search criteria (for MCP integration)
async fn handle_find_instances(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FindInstancesRequest>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4();
    let limit = req.limit.unwrap_or(100).min(1000);
    tracing::info!(
        "find-instances:search request {} - className: {:?}, name: {:?}, parent: {:?}, limit: {}",
        request_id,
        req.class_name,
        req.name,
        req.parent,
        limit
    );
    let request = PluginRequest {
        id: request_id,
        command: "find-instances:search".to_string(),
        payload: serde_json::json!({
            "className": req.class_name,
            "name": req.name,
            "parent": req.parent,
            "limit": limit
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
    tracing::info!(
        "find-instances:search request {} - queued (queue length: {})",
        request_id,
        queue_len
    );
    state.trigger.send(()).ok();

    // Wait for response with timeout (longer for searching large hierarchies)
    let timeout = tokio::time::Duration::from_secs(60);
    match tokio::time::timeout(timeout, rx.recv()).await {
        Ok(Some(response)) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": response.success,
                    "data": response.data,
                    "error": response.error
                })),
            )
        }
        Ok(None) => {
            state.response_channels.write().await.remove(&request_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "data": null,
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
                    "data": null,
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
