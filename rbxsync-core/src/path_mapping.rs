//! File-path to instance-path mapping conventions
//!
//! The single definition of how on-disk files map to DataModel instance
//! paths: script suffix stripping, `_meta.rbxjson`, and the Rojo-style
//! `init.*` parent convention. All disk-to-Studio read paths (file
//! watcher, read-tree, incremental, diff) delegate here so the
//! conventions cannot drift.

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn map(rel: &str) -> MappedPath {
        file_to_instance_path(&PathBuf::from(rel))
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
}
