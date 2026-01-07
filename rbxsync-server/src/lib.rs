//! RbxSync Server
//!
//! HTTP server that communicates with the Roblox Studio plugin
//! for game extraction and synchronization.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use axum::{
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch, Mutex, RwLock};
use uuid::Uuid;

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

/// Shared application state
pub struct AppState {
    /// Queue of pending requests to send to the plugin
    pub request_queue: Mutex<VecDeque<PluginRequest>>,

    /// Map of request ID to response channel
    pub response_channels: RwLock<HashMap<Uuid, mpsc::UnboundedSender<PluginResponse>>>,

    /// Trigger to wake up long-polling requests
    pub trigger: watch::Sender<()>,

    /// Receiver for trigger notifications
    pub trigger_rx: watch::Receiver<()>,

    /// Active extraction session
    pub extraction_session: RwLock<Option<ExtractionSession>>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let (trigger, trigger_rx) = watch::channel(());
        Arc::new(Self {
            request_queue: Mutex::new(VecDeque::new()),
            response_channels: RwLock::new(HashMap::new()),
            trigger,
            trigger_rx,
            extraction_session: RwLock::new(None),
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

/// Create the main router
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // RbxSync plugin communication endpoints (separate from roblox-mcp)
        .route("/rbxsync/request", get(handle_request_poll))
        .route("/rbxsync/response", post(handle_response))
        // New extraction endpoints
        .route("/extract/start", post(handle_extract_start))
        .route("/extract/chunk", post(handle_extract_chunk))
        .route("/extract/status", get(handle_extract_status))
        .route("/extract/export", post(handle_extract_export))
        // Sync endpoints
        .route("/sync/command", post(handle_sync_command))
        .route("/sync/batch", post(handle_sync_batch))
        // Health check
        .route("/health", get(handle_health))
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

/// Long-polling endpoint for plugin to receive requests
async fn handle_request_poll(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // First check if there's already a request in the queue
    {
        let mut queue = state.request_queue.lock().await;
        if let Some(request) = queue.pop_front() {
            return (StatusCode::OK, Json(serde_json::to_value(&request).unwrap()));
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
            let mut queue = state.request_queue.lock().await;
            if let Some(request) = queue.pop_front() {
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
    let channels = state.response_channels.read().await;
    if let Some(sender) = channels.get(&response.id) {
        let _ = sender.send(response);
    }
    StatusCode::OK
}

/// Start extraction request
#[derive(Debug, Deserialize)]
pub struct ExtractStartRequest {
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

    // Queue request to plugin
    let plugin_request = PluginRequest {
        id: session_uuid,
        command: "extract:start".to_string(),
        payload: serde_json::json!({
            "services": req.services.unwrap_or_default(),
            "includeTerrain": req.include_terrain.unwrap_or(true),
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
}

async fn handle_extract_chunk(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExtractChunkRequest>,
) -> impl IntoResponse {
    let mut session_guard = state.extraction_session.write().await;

    // Auto-create session if plugin started extraction directly
    if session_guard.is_none() {
        tracing::info!("Auto-created extraction session: {}", &req.session_id);

        // Create output directory for this session
        let output_dir = format!(".rbxsync/extract_{}", &req.session_id);
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
            tracing::info!("Session ID changed from {} to {}, resetting", session.id, &req.session_id);
            session.id = req.session_id.clone();
            session.chunks_received = 0;
            session.data.clear();

            // Create new output directory
            let output_dir = format!(".rbxsync/extract_{}", &req.session_id);
            let _ = std::fs::create_dir_all(&output_dir);
        }

        session.total_chunks = Some(req.total_chunks);
        session.chunks_received += 1;

        // Save chunk to disk immediately
        let output_dir = format!(".rbxsync/extract_{}", &session.id);
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

/// Start the server
pub async fn run_server(config: ServerConfig) -> anyhow::Result<()> {
    let state = AppState::new();
    let router = create_router(state);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("RbxSync server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
