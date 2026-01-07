//! File watcher module for live sync
//!
//! Watches project directories for file changes and pushes updates to Studio.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, RwLock};

/// File change event
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub project_dir: String,
    pub kind: FileChangeKind,
}

/// Kind of file change
#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeKind {
    Create,
    Modify,
    Delete,
}

/// File watcher state
pub struct FileWatcherState {
    /// Directories being watched
    pub watched_dirs: HashSet<String>,
    /// Debounce tracking (path -> last event time)
    pub pending_changes: HashMap<PathBuf, (Instant, FileChangeKind)>,
    /// Channel to send file changes
    pub change_tx: mpsc::UnboundedSender<FileChange>,
}

impl FileWatcherState {
    pub fn new(change_tx: mpsc::UnboundedSender<FileChange>) -> Self {
        Self {
            watched_dirs: HashSet::new(),
            pending_changes: HashMap::new(),
            change_tx,
        }
    }
}

/// Start the file watcher for a project directory
pub async fn start_file_watcher(
    project_dir: String,
    state: Arc<RwLock<FileWatcherState>>,
) -> anyhow::Result<()> {
    // Check if already watching
    {
        let state = state.read().await;
        if state.watched_dirs.contains(&project_dir) {
            tracing::debug!("Already watching: {}", project_dir);
            return Ok(());
        }
    }

    let src_dir = PathBuf::from(&project_dir).join("src");
    if !src_dir.exists() {
        tracing::warn!("Source directory does not exist: {:?}", src_dir);
        return Ok(());
    }

    tracing::info!("Starting file watcher for: {:?}", src_dir);

    // Mark as watching
    {
        let mut state = state.write().await;
        state.watched_dirs.insert(project_dir.clone());
    }

    let project_dir_clone = project_dir.clone();
    let state_clone = state.clone();

    // Start watcher in a separate task
    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();

        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        ) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(&src_dir, RecursiveMode::Recursive) {
            tracing::error!("Failed to watch directory: {}", e);
            return;
        }

        tracing::info!("File watcher active for: {:?}", src_dir);

        // Process events
        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(event) => {
                    let kind = match event.kind {
                        EventKind::Create(_) => Some(FileChangeKind::Create),
                        EventKind::Modify(_) => Some(FileChangeKind::Modify),
                        EventKind::Remove(_) => Some(FileChangeKind::Delete),
                        _ => None,
                    };

                    if let Some(kind) = kind {
                        for path in event.paths {
                            // Only process .luau and .rbxjson files
                            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                if ext == "luau" || ext == "rbxjson" {
                                    let change = FileChange {
                                        path: path.clone(),
                                        project_dir: project_dir_clone.clone(),
                                        kind: kind.clone(),
                                    };

                                    // Send to async handler
                                    let state = state_clone.clone();
                                    rt.spawn(async move {
                                        let state = state.read().await;
                                        let _ = state.change_tx.send(change);
                                    });
                                }
                            }
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Continue watching
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::info!("File watcher channel closed");
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Process a file change and prepare sync operation
pub fn process_file_change(
    change: &FileChange,
) -> Option<serde_json::Value> {
    let path = &change.path;
    let project_dir = PathBuf::from(&change.project_dir);
    let src_dir = project_dir.join("src");

    // Get relative path from src directory
    let rel_path = match path.strip_prefix(&src_dir) {
        Ok(p) => p,
        Err(_) => return None,
    };

    // Convert to instance path (e.g., "ServerScriptService/MyScript")
    let inst_path = rel_path
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches(".server.luau")
        .trim_end_matches(".client.luau")
        .trim_end_matches(".luau")
        .trim_end_matches(".rbxjson")
        .to_string();

    match change.kind {
        FileChangeKind::Delete => {
            Some(serde_json::json!({
                "type": "delete",
                "path": inst_path,
            }))
        }
        FileChangeKind::Create | FileChangeKind::Modify => {
            // Read the file content
            let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            if file_ext == "luau" {
                // Script file
                let source = match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("Failed to read file {:?}: {}", path, e);
                        return None;
                    }
                };

                // Determine script type from filename
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let class_name = if filename.ends_with(".server.luau") {
                    "Script"
                } else if filename.ends_with(".client.luau") {
                    "LocalScript"
                } else {
                    "ModuleScript"
                };

                Some(serde_json::json!({
                    "type": if change.kind == FileChangeKind::Create { "create" } else { "update" },
                    "path": inst_path,
                    "data": {
                        "className": class_name,
                        "path": inst_path,
                        "properties": {
                            "Source": {
                                "type": "string",
                                "value": source
                            }
                        }
                    }
                }))
            } else if file_ext == "rbxjson" {
                // Instance JSON file
                let content = match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("Failed to read file {:?}: {}", path, e);
                        return None;
                    }
                };

                let data: serde_json::Value = match serde_json::from_str(&content) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!("Failed to parse JSON {:?}: {}", path, e);
                        return None;
                    }
                };

                Some(serde_json::json!({
                    "type": if change.kind == FileChangeKind::Create { "create" } else { "update" },
                    "path": inst_path,
                    "data": data
                }))
            } else {
                None
            }
        }
    }
}
