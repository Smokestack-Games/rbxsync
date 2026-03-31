//! File watcher module for live sync
//!
//! Watches project directories for file changes and pushes updates to Studio.
//! Supports Wally package exclusion to prevent package files from being synced.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify::event::{ModifyKind, DataChange, RenameMode};
use tokio::sync::{mpsc, RwLock};

use rbxsync_core::is_package_path;

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
    Rename { from_path: PathBuf },
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
///
/// If `sync_packages` is true, Wally package changes will be included in file sync.
/// By default, packages are excluded from file watching.
pub async fn start_file_watcher(
    project_dir: String,
    state: Arc<RwLock<FileWatcherState>>,
    sync_packages: bool,
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

        // Buffer for correlating rename From/To event pairs
        let mut pending_rename_from: Option<(PathBuf, Instant)> = None;

        // Helper: determine if a path should be processed based on extension/kind
        let should_process_path = |path: &PathBuf, kind: &FileChangeKind| -> bool {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                ext == "luau" || ext == "rbxjson"
            } else {
                // For deletions and renames, also handle directories (no extension)
                let is_delete_like = matches!(kind, FileChangeKind::Delete | FileChangeKind::Rename { .. });
                if is_delete_like {
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
            }
        };

        // Process events
        loop {
            // Flush stale pending_rename_from (if no matching To arrived within 100ms)
            if let Some((ref from_path, ref timestamp)) = pending_rename_from {
                if timestamp.elapsed() > Duration::from_millis(100) {
                    let kind = FileChangeKind::Delete;
                    let path = from_path.clone();
                    if should_process_path(&path, &kind) {
                        if !sync_packages && is_package_path(&path) {
                            tracing::trace!("Skipping package path (sync_packages=false): {:?}", path);
                        } else {
                            let change = FileChange {
                                path,
                                project_dir: project_dir_clone.clone(),
                                kind,
                            };
                            let state = state_clone.clone();
                            rt.spawn(async move {
                                let state = state.read().await;
                                let _ = state.change_tx.send(change);
                            });
                        }
                    }
                    pending_rename_from = None;
                }
            }

            // Reduced from 1s to 50ms to ensure timely flushing of buffered rename-from events.
            // Trade-off: ~20 wakeups/sec idle vs 1/sec, acceptable for a file watcher.
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(event) => {
                    // Handle RenameMode::Both at event level (some platforms deliver both paths in one event)
                    if let EventKind::Modify(ModifyKind::Name(RenameMode::Both)) = &event.kind {
                        if event.paths.len() == 2 {
                            let from_path = event.paths[0].clone();
                            let to_path = event.paths[1].clone();
                            let kind = FileChangeKind::Rename { from_path: from_path.clone() };

                            // Skip package paths unless sync_packages is enabled
                            if !sync_packages && (is_package_path(&to_path) || is_package_path(&from_path)) {
                                tracing::trace!("Skipping package rename (sync_packages=false): {:?} -> {:?}", from_path, to_path);
                                continue;
                            }

                            if should_process_path(&to_path, &kind) || should_process_path(&from_path, &FileChangeKind::Delete) {
                                let change = FileChange {
                                    path: to_path,
                                    project_dir: project_dir_clone.clone(),
                                    kind,
                                };
                                let state = state_clone.clone();
                                rt.spawn(async move {
                                    let state = state.read().await;
                                    let _ = state.change_tx.send(change);
                                });
                            }
                            continue;
                        }
                    }

                    // Process each path in the event with macOS-aware kind detection
                    for path in event.paths.iter() {
                        // Determine the event kind using Argon's macOS approach:
                        // - Create: only if path exists
                        // - Modify(Name): correlate From/To for renames
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
                                    // Name changes - correlate From/To for rename detection
                                    ModifyKind::Name(rename_mode) => {
                                        match rename_mode {
                                            RenameMode::From => {
                                                // Buffer the From path; don't emit yet
                                                pending_rename_from = Some((path.clone(), Instant::now()));
                                                None
                                            }
                                            RenameMode::To => {
                                                // Check for a buffered From within 100ms
                                                if let Some((from_path, timestamp)) = pending_rename_from.take() {
                                                    if timestamp.elapsed() <= Duration::from_millis(100) {
                                                        Some(FileChangeKind::Rename { from_path })
                                                    } else {
                                                        // Stale From - flush as Delete, then handle To
                                                        let delete_kind = FileChangeKind::Delete;
                                                        if should_process_path(&from_path, &delete_kind)
                                                            && (sync_packages || !is_package_path(&from_path))
                                                        {
                                                            let change = FileChange {
                                                                path: from_path,
                                                                project_dir: project_dir_clone.clone(),
                                                                kind: delete_kind,
                                                            };
                                                            let state = state_clone.clone();
                                                            rt.spawn(async move {
                                                                let state = state.read().await;
                                                                let _ = state.change_tx.send(change);
                                                            });
                                                        }
                                                        // Treat To as Create
                                                        if path.exists() {
                                                            Some(FileChangeKind::Create)
                                                        } else {
                                                            None
                                                        }
                                                    }
                                                } else {
                                                    // No buffered From - fall back to existence check
                                                    if path.exists() {
                                                        Some(FileChangeKind::Create)
                                                    } else {
                                                        Some(FileChangeKind::Delete)
                                                    }
                                                }
                                            }
                                            RenameMode::Both => {
                                                // Already handled at event level for 2-path events;
                                                // if we get here with a single path, fall back
                                                if path.exists() {
                                                    Some(FileChangeKind::Create)
                                                } else {
                                                    Some(FileChangeKind::Delete)
                                                }
                                            }
                                            // RenameMode::Any or other - fall back to existence check
                                            _ => {
                                                if path.exists() {
                                                    Some(FileChangeKind::Create)
                                                } else {
                                                    Some(FileChangeKind::Delete)
                                                }
                                            }
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

                            // Skip system directories to prevent infinite recursion (RBXSYNC-141)
                            let is_system = path.components().any(|c| {
                                if let std::path::Component::Normal(name) = c {
                                    let s = name.to_string_lossy();
                                    s.starts_with(".rbxsync") || s == ".git"
                                } else {
                                    false
                                }
                            });
                            if is_system {
                                continue;
                            }

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

                            // Skip package paths unless sync_packages is enabled
                            if !sync_packages && is_package_path(&path) {
                                tracing::trace!("Skipping package path (sync_packages=false): {:?}", path);
                                continue;
                            }

                            // Check if it's a file we care about
                            if should_process_path(&path, &kind) {
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
            .map(rbxsync_core::path_to_string)
            .unwrap_or_default()
    } else {
        rbxsync_core::path_to_string(rel_path)
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
        FileChangeKind::Rename { ref from_path } => {
            // Compute old instance path from from_path
            let old_rel = match from_path.strip_prefix(&src_dir) {
                Ok(p) => p,
                Err(_) => return None,
            };
            let old_filename = from_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let old_inst_path = if old_filename == "_meta.rbxjson" {
                old_rel
                    .parent()
                    .map(rbxsync_core::path_to_string)
                    .unwrap_or_default()
            } else {
                rbxsync_core::path_to_string(old_rel)
                    .trim_end_matches(".server.luau")
                    .trim_end_matches(".client.luau")
                    .trim_end_matches(".luau")
                    .trim_end_matches(".rbxjson")
                    .to_string()
            };

            // If the new path doesn't exist, treat as a delete of the old path
            if !path.exists() {
                return Some(serde_json::json!({
                    "type": "delete",
                    "path": old_inst_path,
                }));
            }

            Some(serde_json::json!({
                "type": "rename",
                "path": inst_path,
                "data": {
                    "oldPath": old_inst_path,
                    "newPath": inst_path
                }
            }))
        }
    }
}
