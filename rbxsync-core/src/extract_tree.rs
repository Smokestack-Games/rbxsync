//! Extraction tree-write planning
//!
//! Turns a set of extracted Studio instances into the filesystem changes that
//! represent them under a project's `src` directory: which directories to
//! create, which script source files to adopt, and which `.rbxjson` sidecar or
//! `_meta.rbxjson` container files to write.
//!
//! Planning ([`plan_instance_writes`]) is a pure decision pass with only
//! read-only `.exists()` checks (adopt-once script gating); execution is
//! separate so the async server executor and the synchronous CLI executor
//! ([`execute_write_plan_sync`]) can both drive the same plan.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::{apply_tree_mapping, path_with_suffix, pathbuf_with_suffix, SCRIPT_FILE_SUFFIXES};

/// Directories to skip during recursive copy operations
const SKIP_DIRS: &[&str] = &[".rbxsync-trash", ".rbxsync-backup", ".rbxsync", ".git", "node_modules"];

/// A single file to create during extraction.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteOp {
    pub path: PathBuf,
    pub content: String,
}

/// The filesystem changes an extraction should apply, decided without writing.
#[derive(Debug, Clone)]
pub struct WritePlan {
    pub dirs: Vec<PathBuf>,
    pub script_ops: Vec<WriteOp>,
    pub json_ops: Vec<WriteOp>,
    pub adopted: Vec<String>,
    pub service_folders: HashSet<String>,
}

/// Build a `referenceId -> disambiguated_path` map for a flat instance array.
///
/// `dom_to_instances` emits the SAME flat `path` for duplicate-named siblings
/// (only slashes escaped; distinct identity is carried in `referenceId`). To
/// keep such siblings distinct on disk (`.luau`/`.rbxjson` files) and in the
/// context document, the Nth (N > 1) occurrence of a given path gets `_` plus
/// the first 8 chars of its `referenceId` appended. Instances with an empty
/// `path`, or with no `referenceId`, are omitted from the map — callers should
/// fall back to the instance's raw `path` for those.
///
/// This is the single source of truth for path disambiguation, shared by
/// [`plan_instance_writes`] (the `.luau`/`.rbxjson` writers) and the context
/// document assembler, so the two always agree on a duplicate's path.
pub fn disambiguate_paths(instances: &[serde_json::Value]) -> HashMap<String, String> {
    let mut path_to_count: HashMap<String, usize> = HashMap::new();
    let mut ref_to_path: HashMap<String, String> = HashMap::new();
    let mut duplicate_count = 0;

    for inst in instances {
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

    ref_to_path
}

/// Decide the directories, script adoptions, and `.rbxjson` writes for a set of
/// extracted instances, resolving duplicate sibling paths and honoring
/// adopt-once (a script is only planned when no script file already exists at
/// its target path). Performs read-only `.exists()` checks; writes nothing.
pub fn plan_instance_writes(
    src_dir: &Path,
    instances: &[serde_json::Value],
    tree_mapping: &HashMap<String, String>,
) -> WritePlan {
    // First pass: build a map from referenceId to disambiguated path.
    // This handles duplicate sibling names by appending a referenceId suffix,
    // shared with the context document assembler via disambiguate_paths.
    let ref_to_path = disambiguate_paths(instances);

    // Collect all disambiguated paths for container detection
    let all_paths: HashSet<String> = ref_to_path.values().cloned().collect();

    // A path is a container when another path begins with `path + "/"`. Insert
    // each path's ancestor prefixes once so container detection is a hash lookup
    // rather than a scan of every path for every instance.
    let mut container_paths: HashSet<&str> = HashSet::new();
    for path in &all_paths {
        let mut idx = path.len();
        while let Some(slash) = path[..idx].rfind('/') {
            container_paths.insert(&path[..slash]);
            idx = slash;
        }
    }

    // Helper to check if a path has children (is a container)
    let has_children = |path: &str| -> bool { container_paths.contains(path) };

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

    let mut directories_needed: HashSet<PathBuf> = HashSet::new();
    let mut script_write_ops: Vec<WriteOp> = Vec::new();
    let mut json_write_ops: Vec<WriteOp> = Vec::new();
    let mut adopted: Vec<String> = Vec::new();
    let mut service_folders: HashSet<String> = HashSet::new();

    for inst in instances {
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
        let fs_path = apply_tree_mapping(&inst_path, tree_mapping);

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
            if let Some(props) = inst.get("properties") {
                if let Some(source) = props.get("Source").and_then(|v| v.get("value")).and_then(|v| v.as_str()) {
                    // Scripts are managed on disk: write only when no script
                    // file exists at the target path (adopt-once)
                    let script_exists = SCRIPT_FILE_SUFFIXES.iter().any(|ext| {
                        PathBuf::from(path_with_suffix(&full_path, ext)).exists()
                    });
                    if !script_exists {
                        let extension = match class_name {
                            "Script" => ".server.luau",
                            "LocalScript" => ".client.luau",
                            _ => ".luau",
                        };
                        let script_path = path_with_suffix(&full_path, extension);
                        adopted.push(format!("{}{}", fs_path, extension));
                        script_write_ops.push(WriteOp {
                            path: PathBuf::from(script_path),
                            content: source.to_string(),
                        });
                    }
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
            pathbuf_with_suffix(&full_path, ".rbxjson")
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

    WritePlan {
        dirs: directories_needed.into_iter().collect(),
        script_ops: script_write_ops,
        json_ops: json_write_ops,
        adopted,
        service_folders,
    }
}

/// Plan just the adopt-once `.luau` script writes for a set of instances
/// (no per-instance json). Delegates to `plan_instance_writes` so script path
/// resolution, disambiguation, and adopt-once stay identical to a full extraction.
pub fn plan_script_writes(
    src_dir: &Path,
    instances: &[serde_json::Value],
    tree_mapping: &HashMap<String, String>,
) -> Vec<WriteOp> {
    plan_instance_writes(src_dir, instances, tree_mapping).script_ops
}

/// Execute a [`WritePlan`] synchronously with `std::fs`, creating directories
/// then writing scripts and json files. Returns `(files_written, scripts_written)`.
pub fn execute_write_plan_sync(plan: &WritePlan) -> (usize, usize) {
    for dir in &plan.dirs {
        let _ = std::fs::create_dir_all(dir);
    }
    let scripts_written = plan
        .script_ops
        .iter()
        .filter(|op| std::fs::write(&op.path, &op.content).is_ok())
        .count();
    let files_written = plan
        .json_ops
        .iter()
        .filter(|op| std::fs::write(&op.path, &op.content).is_ok())
        .count();
    (files_written, scripts_written)
}

/// Recursively copy a directory, skipping system directories and
/// preventing circular copies (dst inside src).
pub fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    let resolved_src = src.canonicalize().unwrap_or_else(|_| src.clone());
    let resolved_dst = dst.canonicalize().unwrap_or_else(|_| dst.clone());

    if resolved_dst.starts_with(&resolved_src) {
        tracing::warn!("Skipping circular copy: {:?} is inside {:?}", dst, src);
        return Ok(());
    }

    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                if SKIP_DIRS.contains(&name) {
                    continue;
                }
            }
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
}

/// Delete instance data files (.rbxjson, including _meta.rbxjson) under `dir`
/// and remove directories left empty; script source files are preserved.
/// Returns the absolute paths of the files removed.
pub fn clear_instance_files(dir: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Ok(mut inner) = clear_instance_files(&path) {
                removed.append(&mut inner);
            }
            // Succeeds only when empty; non-empty directories are kept
            let _ = std::fs::remove_dir(&path);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rbxjson")
            && std::fs::remove_file(&path).is_ok()
        {
            removed.push(path);
        }
    }
    Ok(removed)
}

/// Back up src to .rbxsync-backup/src and clear instance data files,
/// preserving script sources in place.
pub fn prepare_src_for_extraction(project_dir: &str) {
    let src_dir = PathBuf::from(project_dir).join("src");
    let backup_dir = PathBuf::from(project_dir).join(".rbxsync-backup");
    let backup_src = backup_dir.join("src");

    if src_dir.exists() {
        if backup_src.exists() {
            let _ = std::fs::remove_dir_all(&backup_src);
        }
        let _ = std::fs::create_dir_all(&backup_dir);
        if let Err(e) = copy_dir_recursive(&src_dir, &backup_src) {
            tracing::warn!("Failed to back up src directory: {}", e);
        } else {
            tracing::info!("Backed up src to .rbxsync-backup/src");
        }
        let _ = clear_instance_files(&src_dir);
    }
    let _ = std::fs::create_dir_all(&src_dir);
}

/// Record snapshot freshness in <project>/.rbxsync/snapshot.json (epoch millis).
/// `full_extract` also stamps lastFullExtract; every call stamps lastLiveUpdate.
pub fn write_snapshot_freshness(project_dir: &str, full_extract: bool) {
    let dir = PathBuf::from(project_dir).join(".rbxsync");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("snapshot.json");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let mut doc = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = doc.as_object_mut() {
        if full_extract {
            obj.insert("lastFullExtract".to_string(), serde_json::json!(now_ms));
        }
        obj.insert("lastLiveUpdate".to_string(), serde_json::json!(now_ms));
    }

    let tmp = dir.join("snapshot.json.tmp");
    if let Ok(json) = serde_json::to_string_pretty(&doc) {
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn part(path: &str) -> serde_json::Value {
        json!({ "className": "Part", "name": path.rsplit('/').next().unwrap(),
                "path": path, "referenceId": format!("R{}", path), "properties": {} })
    }
    fn script(path: &str, src: &str) -> serde_json::Value {
        json!({ "className": "Script", "name": path.rsplit('/').next().unwrap(),
                "path": path, "referenceId": format!("R{}", path),
                "properties": { "Source": { "type": "string", "value": src } } })
    }

    #[test]
    fn test_plan_leaf_and_container_and_script() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let insts = vec![
            part("Workspace/Model"),         // container (has child below)
            part("Workspace/Model/Leaf"),    // leaf
            script("ServerScriptService/Main", "print('x')"),
        ];
        let plan = plan_instance_writes(&src, &insts, &std::collections::HashMap::new());
        let (files, scripts) = execute_write_plan_sync(&plan);
        assert!(src.join("Workspace/Model/_meta.rbxjson").exists());
        assert!(src.join("Workspace/Model/Leaf.rbxjson").exists());
        assert!(src.join("ServerScriptService/Main.server.luau").exists());
        // Sidecar written, source stripped
        let sidecar = std::fs::read_to_string(src.join("ServerScriptService/Main.rbxjson")).unwrap();
        assert!(!sidecar.contains("\"Source\""));
        assert_eq!(scripts, 1);
        assert_eq!(plan.adopted, vec!["ServerScriptService/Main.server.luau"]);
        assert!(files >= 3);
    }

    #[test]
    fn test_adopt_once_skips_existing_script() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("ServerScriptService")).unwrap();
        std::fs::write(src.join("ServerScriptService/Main.server.luau"), "-- local").unwrap();
        let plan = plan_instance_writes(&src, &[script("ServerScriptService/Main", "print('studio')")],
            &std::collections::HashMap::new());
        assert!(plan.adopted.is_empty());
        assert!(plan.script_ops.is_empty());
        execute_write_plan_sync(&plan);
        assert_eq!(std::fs::read_to_string(src.join("ServerScriptService/Main.server.luau")).unwrap(), "-- local");
    }

    #[test]
    fn test_plan_script_writes_adopt_once() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("ServerScriptService")).unwrap();
        let insts = vec![
            serde_json::json!({"className":"Script","name":"Main","path":"ServerScriptService/Main",
                "properties":{"Source":{"type":"string","value":"print('x')"}}}),
            serde_json::json!({"className":"Part","name":"P","path":"Workspace/P","properties":{}}),
        ];
        let ops = plan_script_writes(&src, &insts, &std::collections::HashMap::new());
        assert_eq!(ops.len(), 1);
        assert!(ops[0].path.to_string_lossy().ends_with("Main.server.luau"));
        // Adopt-once: pre-existing script is not re-planned
        std::fs::write(src.join("ServerScriptService/Main.server.luau"), "-- mine").unwrap();
        assert!(plan_script_writes(&src, &insts, &std::collections::HashMap::new()).is_empty());
    }

    #[test]
    fn test_plan_script_writes_matches_plan_instance_writes_script_ops() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let tm = std::collections::HashMap::new();
        let insts = vec![
            // a script under a duplicated Packages path (exercises the dedup normalize)
            serde_json::json!({"className":"ModuleScript","name":"Lib","path":"ReplicatedStorage/Packages/Packages/Lib",
                "referenceId":"a1","properties":{"Source":{"type":"string","value":"return 1"}}}),
            // duplicate-named script siblings (exercises disambiguation)
            serde_json::json!({"className":"Script","name":"Run","path":"ServerScriptService/Run",
                "referenceId":"b1","properties":{"Source":{"type":"string","value":"print(1)"}}}),
            serde_json::json!({"className":"Script","name":"Run","path":"ServerScriptService/Run",
                "referenceId":"b2","properties":{"Source":{"type":"string","value":"print(2)"}}}),
            serde_json::json!({"className":"Part","name":"P","path":"Workspace/P","referenceId":"c1","properties":{}}),
        ];
        let via_scripts = plan_script_writes(&src, &insts, &tm);
        let via_full = plan_instance_writes(&src, &insts, &tm).script_ops;
        assert_eq!(via_scripts, via_full, "plan_script_writes must equal plan_instance_writes.script_ops");
        // sanity: three scripts planned (non-script Part excluded)
        assert_eq!(via_scripts.len(), 3);
    }
}
