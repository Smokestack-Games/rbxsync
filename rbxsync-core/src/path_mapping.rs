//! File-path to instance-path mapping conventions
//!
//! The single definition of how on-disk files map to DataModel instance
//! paths: script suffix stripping, `_meta.rbxjson`, and the Rojo-style
//! `init.*` parent convention. All disk-to-Studio read paths (file
//! watcher, read-tree, incremental, diff) delegate here so the
//! conventions cannot drift.

use std::collections::HashMap;
use std::path::Path;

use crate::path_utils::path_to_string;

/// Script class implied by a filename's suffix convention
pub fn script_class_from_filename(filename: &str) -> &'static str {
    if filename.ends_with(".server.luau") || filename.ends_with(".server.lua") {
        "Script"
    } else if filename.ends_with(".client.luau") || filename.ends_with(".client.lua") {
        "LocalScript"
    } else {
        "ModuleScript"
    }
}

/// A file path mapped to its DataModel instance path
#[derive(Debug, Clone, PartialEq)]
pub struct MappedPath {
    /// Instance path with `/` separators (e.g. "ServerScriptService/Main")
    pub instance_path: String,
    /// Script class for `.luau`/`.lua` files, `None` otherwise
    pub script_class: Option<&'static str>,
}

/// Script source file suffixes, most specific first
pub const SCRIPT_FILE_SUFFIXES: [&str; 6] = [
    ".server.luau",
    ".client.luau",
    ".luau",
    ".server.lua",
    ".client.lua",
    ".lua",
];

/// Map a src-relative file path to its DataModel instance path.
///
/// `_meta.rbxjson` and `init.*` files represent their parent directory.
/// Script suffixes (`.server.luau`, `.client.lua`, ...) and `.rbxjson`
/// are stripped. Paths without a recognized extension (directories) map
/// to themselves.
pub fn file_to_instance_path(rel_path: &Path) -> MappedPath {
    let filename = rel_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let is_script_file = matches!(
        rel_path.extension().and_then(|e| e.to_str()),
        Some("luau") | Some("lua")
    );
    let script_class = is_script_file.then(|| script_class_from_filename(filename));

    if filename == "_meta.rbxjson" || filename.starts_with("init.") {
        let instance_path = rel_path.parent().map(path_to_string).unwrap_or_default();
        return MappedPath { instance_path, script_class };
    }

    let instance_path = path_to_string(rel_path)
        .trim_end_matches(".server.luau")
        .trim_end_matches(".client.luau")
        .trim_end_matches(".server.lua")
        .trim_end_matches(".client.lua")
        .trim_end_matches(".luau")
        .trim_end_matches(".lua")
        .trim_end_matches(".rbxjson")
        .to_string();
    MappedPath { instance_path, script_class }
}

/// Apply tree mapping to convert DataModel path to filesystem path
pub fn apply_tree_mapping(datamodel_path: &str, tree_mapping: &HashMap<String, String>) -> String {
    // Try to find longest matching prefix
    let mut best_match: Option<(&str, &str)> = None;
    let mut best_len = 0;

    for (dm_prefix, fs_prefix) in tree_mapping {
        if (datamodel_path == dm_prefix || datamodel_path.starts_with(&format!("{}/", dm_prefix)))
            && dm_prefix.len() > best_len {
                best_match = Some((dm_prefix.as_str(), fs_prefix.as_str()));
                best_len = dm_prefix.len();
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
pub fn apply_reverse_tree_mapping(fs_path: &str, tree_mapping: &HashMap<String, String>) -> String {
    // Try to find longest matching prefix (reverse lookup)
    let mut best_match: Option<(&str, &str)> = None;
    let mut best_len = 0;

    for (dm_prefix, fs_prefix) in tree_mapping {
        if (fs_path == fs_prefix || fs_path.starts_with(&format!("{}/", fs_prefix)))
            && fs_prefix.len() > best_len {
                best_match = Some((dm_prefix.as_str(), fs_prefix.as_str()));
                best_len = fs_prefix.len();
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

/// Read the treeMapping from a project's rbxsync.json
/// (DataModel path prefix -> src-relative filesystem prefix)
pub fn load_tree_mapping(project_dir: &Path) -> HashMap<String, String> {
    let config_path = project_dir.join("rbxsync.json");
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return HashMap::new();
    };
    let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) else {
        return HashMap::new();
    };
    tree_mapping_from_config(Some(&config))
}

/// Extract treeMapping from a project config JSON value
pub fn tree_mapping_from_config(config: Option<&serde_json::Value>) -> HashMap<String, String> {
    config
        .and_then(|c| c.get("treeMapping"))
        .and_then(|m| m.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn map(rel: &str) -> MappedPath {
        file_to_instance_path(&PathBuf::from(rel))
    }

    fn mapping() -> HashMap<String, String> {
        HashMap::from([
            ("ReplicatedStorage/Shared".to_string(), "shared".to_string()),
            ("ReplicatedStorage/Shared/Net".to_string(), "net".to_string()),
        ])
    }

    #[test]
    fn test_apply_tree_mapping() {
        let m = mapping();
        assert_eq!(apply_tree_mapping("ReplicatedStorage/Shared", &m), "shared");
        assert_eq!(apply_tree_mapping("ReplicatedStorage/Shared/Util", &m), "shared/Util");
        // Longest prefix wins
        assert_eq!(apply_tree_mapping("ReplicatedStorage/Shared/Net/Rpc", &m), "net/Rpc");
        // No match is identity
        assert_eq!(apply_tree_mapping("Workspace/Part", &m), "Workspace/Part");
    }

    #[test]
    fn test_apply_reverse_tree_mapping() {
        let m = mapping();
        assert_eq!(apply_reverse_tree_mapping("shared", &m), "ReplicatedStorage/Shared");
        assert_eq!(apply_reverse_tree_mapping("shared/Util", &m), "ReplicatedStorage/Shared/Util");
        assert_eq!(apply_reverse_tree_mapping("net/Rpc", &m), "ReplicatedStorage/Shared/Net/Rpc");
        assert_eq!(apply_reverse_tree_mapping("Workspace/Part", &m), "Workspace/Part");
    }

    #[test]
    fn test_load_tree_mapping() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rbxsync.json"),
            r#"{"treeMapping": {"ReplicatedStorage/Shared": "shared"}}"#,
        )
        .unwrap();
        let m = load_tree_mapping(dir.path());
        assert_eq!(m.get("ReplicatedStorage/Shared").map(String::as_str), Some("shared"));
        // Missing config file yields an empty mapping
        let empty_dir = tempfile::tempdir().unwrap();
        assert!(load_tree_mapping(empty_dir.path()).is_empty());
    }

    #[test]
    fn test_tree_mapping_from_config() {
        let config = serde_json::json!({"treeMapping": {"ReplicatedStorage/Shared": "shared"}});
        let m = tree_mapping_from_config(Some(&config));
        assert_eq!(m.get("ReplicatedStorage/Shared").map(String::as_str), Some("shared"));
        assert!(tree_mapping_from_config(None).is_empty());
        let no_field = serde_json::json!({});
        assert!(tree_mapping_from_config(Some(&no_field)).is_empty());
    }

    #[test]
    fn test_script_class_from_filename() {
        assert_eq!(script_class_from_filename("Main.server.luau"), "Script");
        assert_eq!(script_class_from_filename("Main.server.lua"), "Script");
        assert_eq!(script_class_from_filename("Ctl.client.luau"), "LocalScript");
        assert_eq!(script_class_from_filename("Ctl.client.lua"), "LocalScript");
        assert_eq!(script_class_from_filename("Utils.luau"), "ModuleScript");
        assert_eq!(script_class_from_filename("Utils.lua"), "ModuleScript");
        assert_eq!(script_class_from_filename("init.luau"), "ModuleScript");
        assert_eq!(script_class_from_filename("init.server.luau"), "Script");
    }

    #[test]
    fn test_server_script_suffixes() {
        for rel in ["ServerScriptService/Main.server.luau", "ServerScriptService/Main.server.lua"] {
            let m = map(rel);
            assert_eq!(m.instance_path, "ServerScriptService/Main", "{rel}");
            assert_eq!(m.script_class, Some("Script"), "{rel}");
        }
    }

    #[test]
    fn test_client_script_suffixes() {
        for rel in ["StarterPlayer/Ctl.client.luau", "StarterPlayer/Ctl.client.lua"] {
            let m = map(rel);
            assert_eq!(m.instance_path, "StarterPlayer/Ctl", "{rel}");
            assert_eq!(m.script_class, Some("LocalScript"), "{rel}");
        }
    }

    #[test]
    fn test_module_script_suffixes() {
        for rel in ["ReplicatedStorage/Utils.luau", "ReplicatedStorage/Utils.lua"] {
            let m = map(rel);
            assert_eq!(m.instance_path, "ReplicatedStorage/Utils", "{rel}");
            assert_eq!(m.script_class, Some("ModuleScript"), "{rel}");
        }
    }

    #[test]
    fn test_rbxjson_leaf() {
        let m = map("Workspace/Part.rbxjson");
        assert_eq!(m.instance_path, "Workspace/Part");
        assert_eq!(m.script_class, None);
    }

    #[test]
    fn test_meta_rbxjson_maps_to_parent() {
        let m = map("Workspace/Container/_meta.rbxjson");
        assert_eq!(m.instance_path, "Workspace/Container");
        assert_eq!(m.script_class, None);
    }

    #[test]
    fn test_root_meta_maps_to_empty() {
        assert_eq!(map("_meta.rbxjson").instance_path, "");
    }

    #[test]
    fn test_init_files_map_to_parent() {
        let m = map("ReplicatedStorage/Mod/init.luau");
        assert_eq!(m.instance_path, "ReplicatedStorage/Mod");
        assert_eq!(m.script_class, Some("ModuleScript"));

        let m = map("ServerScriptService/Svc/init.server.luau");
        assert_eq!(m.instance_path, "ServerScriptService/Svc");
        assert_eq!(m.script_class, Some("Script"));

        let m = map("StarterGui/Ui/init.client.lua");
        assert_eq!(m.instance_path, "StarterGui/Ui");
        assert_eq!(m.script_class, Some("LocalScript"));

        let m = map("Workspace/Model/init.rbxjson");
        assert_eq!(m.instance_path, "Workspace/Model");
        assert_eq!(m.script_class, None);
    }

    #[test]
    fn test_directory_maps_to_itself() {
        let m = map("Workspace/SomeFolder");
        assert_eq!(m.instance_path, "Workspace/SomeFolder");
        assert_eq!(m.script_class, None);
    }

    #[test]
    fn test_backslash_separators_normalized() {
        let m = file_to_instance_path(&PathBuf::from(r"Workspace\Deep\Part.rbxjson"));
        assert_eq!(m.instance_path, "Workspace/Deep/Part");
    }

    #[test]
    fn test_unknown_extension_maps_to_itself() {
        let m = map("Workspace/notes.txt");
        assert_eq!(m.instance_path, "Workspace/notes.txt");
        assert_eq!(m.script_class, None);
    }

    #[test]
    fn test_repeated_trailing_suffix_stripped() {
        // trim_end_matches semantics: repeated trailing occurrences are all removed
        let m = map("Workspace/Part.rbxjson.rbxjson");
        assert_eq!(m.instance_path, "Workspace/Part");
        // Non-trailing occurrences are untouched
        let m = map("Workspace/Part.rbxjson.bak");
        assert_eq!(m.instance_path, "Workspace/Part.rbxjson.bak");
    }

    #[test]
    fn test_script_file_suffixes_order() {
        assert_eq!(SCRIPT_FILE_SUFFIXES.len(), 6);
        // Most specific first so suffix matching never truncates partially
        assert_eq!(SCRIPT_FILE_SUFFIXES[0], ".server.luau");
        assert_eq!(SCRIPT_FILE_SUFFIXES[5], ".lua");
    }
}
