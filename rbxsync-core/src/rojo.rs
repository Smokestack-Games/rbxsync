//! Rojo project.json parsing and conversion
//!
//! This module provides functionality to parse Rojo project files
//! and convert them to RbxSync's tree_mapping format for compatibility.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Rojo project.json structure
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RojoProject {
    /// Project name
    pub name: String,

    /// Root tree definition
    pub tree: RojoTree,

    /// Optional: paths to ignore during sync
    #[serde(default)]
    pub glob_ignore_paths: Vec<String>,

    /// Optional: serve host
    #[serde(default)]
    pub serve_address: Option<String>,

    /// Optional: serve port
    #[serde(default)]
    pub serve_port: Option<u16>,
}

/// Rojo tree node structure
///
/// Rojo uses special keys prefixed with `$` for metadata:
/// - `$className`: The Roblox class name
/// - `$path`: Path to file/folder on disk
/// - `$properties`: Property overrides
/// - `$ignoreUnknownInstances`: Whether to ignore unknown instances
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RojoTree {
    /// Class name of this instance
    #[serde(rename = "$className")]
    pub class_name: Option<String>,

    /// Path to file/folder on disk
    #[serde(rename = "$path")]
    pub path: Option<String>,

    /// Property overrides
    #[serde(rename = "$properties")]
    pub properties: Option<HashMap<String, serde_json::Value>>,

    /// Whether to ignore unknown instances
    #[serde(rename = "$ignoreUnknownInstances")]
    pub ignore_unknown_instances: Option<bool>,

    /// Child nodes - all keys not starting with `$`
    #[serde(flatten)]
    pub children: HashMap<String, RojoTree>,
}

/// Error types for Rojo parsing
#[derive(Debug, thiserror::Error)]
pub enum RojoError {
    #[error("Failed to read Rojo project file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse Rojo project file: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Rojo project file not found: {0}")]
    NotFound(String),
}

/// Parse a Rojo project.json file
pub fn parse_rojo_project(path: &Path) -> Result<RojoProject, RojoError> {
    let content = std::fs::read_to_string(path)?;
    let project: RojoProject = serde_json::from_str(&content)?;
    Ok(project)
}

/// Find a Rojo project file in a directory
///
/// Searches for:
/// 1. `default.project.json`
/// 2. Any `*.project.json` file
pub fn find_rojo_project(dir: &Path) -> Result<std::path::PathBuf, RojoError> {
    // First try default.project.json
    let default_path = dir.join("default.project.json");
    if default_path.exists() {
        return Ok(default_path);
    }

    // Then search for any *.project.json
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".project.json") {
                    return Ok(path);
                }
            }
        }
    }

    Err(RojoError::NotFound(format!(
        "No Rojo project file found in {}",
        dir.display()
    )))
}

/// Convert a Rojo project to RbxSync tree_mapping format
///
/// The tree_mapping maps DataModel paths (e.g., "ServerScriptService")
/// to filesystem paths (e.g., "src/server").
///
/// # Example
///
/// Given a Rojo project with:
/// ```json
/// {
///   "tree": {
///     "$className": "DataModel",
///     "ServerScriptService": {
///       "$path": "src/server"
///     },
///     "ReplicatedStorage": {
///       "$path": "src/shared"
///     }
///   }
/// }
/// ```
///
/// This produces:
/// ```text
/// {
///   "ServerScriptService": "src/server",
///   "ReplicatedStorage": "src/shared"
/// }
/// ```
pub fn rojo_to_tree_mapping(project: &RojoProject) -> HashMap<String, String> {
    let mut mapping = HashMap::new();
    walk_tree(&project.tree, "", &mut mapping);
    mapping
}

/// Recursively walk the Rojo tree and extract path mappings
fn walk_tree(tree: &RojoTree, datamodel_path: &str, mapping: &mut HashMap<String, String>) {
    // If this node has a $path, add it to the mapping
    if let Some(fs_path) = &tree.path {
        if !datamodel_path.is_empty() {
            // Normalize the path (remove leading ./ if present)
            let normalized_path = fs_path
                .strip_prefix("./")
                .unwrap_or(fs_path)
                .to_string();
            mapping.insert(datamodel_path.to_string(), normalized_path);
        }
    }

    // Process children (excluding $ keys which are handled by serde)
    for (name, child) in &tree.children {
        // Skip keys that somehow got through (shouldn't happen with serde)
        if name.starts_with('$') {
            continue;
        }

        // Build the child's DataModel path
        let child_path = if datamodel_path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", datamodel_path, name)
        };

        walk_tree(child, &child_path, mapping);
    }
}

/// Get the source directory from a Rojo project
///
/// Returns the most commonly used source path, typically "src"
pub fn get_source_dir(project: &RojoProject) -> Option<String> {
    // Common patterns to look for
    let common_services = [
        "ServerScriptService",
        "ReplicatedStorage",
        "StarterPlayer",
        "StarterGui",
    ];

    for service in &common_services {
        if let Some(child) = project.tree.children.get(*service) {
            if let Some(path) = &child.path {
                // Extract the base directory (e.g., "src/server" -> "src")
                let normalized = path.strip_prefix("./").unwrap_or(path);
                if let Some(base) = normalized.split('/').next() {
                    return Some(base.to_string());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_project() {
        let json = r#"{
            "name": "TestProject",
            "tree": {
                "$className": "DataModel",
                "ServerScriptService": {
                    "$path": "src/server"
                },
                "ReplicatedStorage": {
                    "$path": "src/shared"
                }
            }
        }"#;

        let project: RojoProject = serde_json::from_str(json).unwrap();
        assert_eq!(project.name, "TestProject");
        assert!(project.tree.children.contains_key("ServerScriptService"));
    }

    #[test]
    fn test_rojo_to_tree_mapping() {
        let json = r#"{
            "name": "TestProject",
            "tree": {
                "$className": "DataModel",
                "ServerScriptService": {
                    "$path": "./src/server"
                },
                "ReplicatedStorage": {
                    "$path": "src/shared"
                },
                "StarterPlayer": {
                    "StarterPlayerScripts": {
                        "$path": "src/client"
                    }
                }
            }
        }"#;

        let project: RojoProject = serde_json::from_str(json).unwrap();
        let mapping = rojo_to_tree_mapping(&project);

        assert_eq!(mapping.get("ServerScriptService"), Some(&"src/server".to_string()));
        assert_eq!(mapping.get("ReplicatedStorage"), Some(&"src/shared".to_string()));
        assert_eq!(mapping.get("StarterPlayer/StarterPlayerScripts"), Some(&"src/client".to_string()));
    }

    #[test]
    fn test_nested_paths() {
        let json = r#"{
            "name": "NestedProject",
            "tree": {
                "$className": "DataModel",
                "ReplicatedStorage": {
                    "$className": "ReplicatedStorage",
                    "Shared": {
                        "$path": "src/shared"
                    },
                    "Packages": {
                        "$path": "Packages"
                    }
                }
            }
        }"#;

        let project: RojoProject = serde_json::from_str(json).unwrap();
        let mapping = rojo_to_tree_mapping(&project);

        assert_eq!(mapping.get("ReplicatedStorage/Shared"), Some(&"src/shared".to_string()));
        assert_eq!(mapping.get("ReplicatedStorage/Packages"), Some(&"Packages".to_string()));
    }

    #[test]
    fn test_get_source_dir() {
        let json = r#"{
            "name": "TestProject",
            "tree": {
                "$className": "DataModel",
                "ServerScriptService": {
                    "$path": "src/server"
                }
            }
        }"#;

        let project: RojoProject = serde_json::from_str(json).unwrap();
        assert_eq!(get_source_dir(&project), Some("src".to_string()));
    }
}
