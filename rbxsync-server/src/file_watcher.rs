//! File watcher module for live sync
//!
//! Watches project directories for file changes and pushes updates to Studio.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify::event::{ModifyKind, DataChange};
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
                    // Process each path in the event with macOS-aware kind detection
                    for path in event.paths.iter() {
                        // Determine the event kind using Argon's macOS approach:
                        // - Create: only if path exists
                        // - Modify(Name): check path existence (deletion on macOS comes as rename)
                        // - Modify(Data(Content)): actual content change
                        // - Remove: always delete
                        let kind = match &event.kind {
                            EventKind::Create(_) => {
                                // Only emit Create if the path actually exists
                                if path.exists() {
                                    Some(FileChangeKind::Create)
                                } else {
                                    None
                                }
                            }
                            EventKind::Remove(_) => Some(FileChangeKind::Delete),
                            EventKind::Modify(modify_kind) => {
                                match modify_kind {
                                    // Name changes (rename/move) - check if path exists
                                    // On macOS, deletions often come through as rename events
                                    ModifyKind::Name(_) => {
                                        if path.exists() {
                                            Some(FileChangeKind::Create)
                                        } else {
                                            Some(FileChangeKind::Delete)
                                        }
                                    }
                                    // Data changes - only care about content changes
                                    ModifyKind::Data(data_change) => {
                                        if *data_change == DataChange::Content {
                                            // Verify file still exists (another macOS quirk)
                                            if path.exists() {
                                                Some(FileChangeKind::Modify)
                                            } else {
                                                Some(FileChangeKind::Delete)
                                            }
                                        } else {
                                            None
                                        }
                                    }
                                    // Any other modify - check existence as fallback
                                    ModifyKind::Any => {
                                        if path.exists() {
                                            Some(FileChangeKind::Modify)
                                        } else {
                                            Some(FileChangeKind::Delete)
                                        }
                                    }
                                    // Ignore metadata-only changes
                                    _ => None,
                                }
                            }
                            _ => None,
                        };

                        if let Some(kind) = kind {
                            let path = path.clone();
                            // Check if it's a directory that was created (for undo operations)
                            if kind == FileChangeKind::Create && path.is_dir() {
                                // Scan directory for script files and send Create events for each
                                if let Ok(entries) = std::fs::read_dir(&path) {
                                    for entry in entries.flatten() {
                                        let entry_path = entry.path();
                                        if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                                            if ext == "luau" || ext == "rbxjson" {
                                                let change = FileChange {
                                                    path: entry_path,
                                                    project_dir: project_dir_clone.clone(),
                                                    kind: FileChangeKind::Create,
                                                };
                                                let state = state_clone.clone();
                                                rt.spawn(async move {
                                                    let state = state.read().await;
                                                    let _ = state.change_tx.send(change);
                                                });
                                            }
                                        }
                                    }
                                }
                                continue;
                            }

                            // Check if it's a file we care about
                            let should_process = if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                ext == "luau" || ext == "rbxjson"
                            } else {
                                // For deletions, also handle directories (no extension)
                                // Check that path is inside src (has at least one segment after src)
                                // and doesn't have a dot in the filename (not a file without extension)
                                if kind == FileChangeKind::Delete {
                                    // Make sure we're deleting something INSIDE src, not src itself
                                    let is_inside_src = path.strip_prefix(&src_dir)
                                        .map(|rel| !rel.as_os_str().is_empty())
                                        .unwrap_or(false);
                                    is_inside_src && path.file_name()
                                        .and_then(|n| n.to_str())
                                        .map(|n| !n.contains('.'))
                                        .unwrap_or(false)
                                } else {
                                    false
                                }
                            };

                            if should_process {
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
    // Handle _meta.rbxjson specially - it represents the parent folder
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let inst_path = if filename == "_meta.rbxjson" {
        // _meta.rbxjson represents the parent folder
        rel_path
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default()
    } else {
        rel_path
            .to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches(".server.luau")
            .trim_end_matches(".client.luau")
            .trim_end_matches(".luau")
            .trim_end_matches(".rbxjson")
            .to_string()
    };

    match change.kind {
        FileChangeKind::Delete => {
            // For folder deletions, the path won't have an extension
            // The inst_path will be the folder path in the instance tree
            Some(serde_json::json!({
                "type": "delete",
                "path": inst_path,
                "isFolder": path.extension().is_none(),
            }))
        }
        FileChangeKind::Create | FileChangeKind::Modify => {
            // Check if file still exists (macOS reports deletions as Modify events)
            if !path.exists() {
                // File was deleted - treat as delete
                return Some(serde_json::json!({
                    "type": "delete",
                    "path": inst_path,
                }));
            }

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

                // Extract instance name from path (last segment)
                let instance_name = inst_path.rsplit('/').next().unwrap_or(&inst_path);

                Some(serde_json::json!({
                    "type": if change.kind == FileChangeKind::Create { "create" } else { "update" },
                    "path": inst_path,
                    "data": {
                        "className": class_name,
                        "name": instance_name,
                        "path": inst_path,
                        "source": source,
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

                let mut data: serde_json::Value = match serde_json::from_str(&content) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!("Failed to parse JSON {:?}: {}", path, e);
                        return None;
                    }
                };

                // Ensure path is set from file location (used for tracking, not naming)
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("path".to_string(), serde_json::Value::String(inst_path.clone()));
                    // If no name provided, derive from path
                    if !obj.contains_key("name") {
                        if let Some(name) = inst_path.rsplit('/').next() {
                            obj.insert("name".to_string(), serde_json::Value::String(name.to_string()));
                        }
                    }
                }

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
