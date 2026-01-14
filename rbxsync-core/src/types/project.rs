//! RbxSync project configuration
//!
//! Defines the `rbxsync.json` manifest format.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// The main project configuration file (rbxsync.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    /// Project name (used for display and as default place name)
    pub name: String,

    /// Path to the source tree (default: "./src")
    #[serde(default = "default_tree_path")]
    pub tree: PathBuf,

    /// Path to binary assets (default: "./assets")
    #[serde(default = "default_assets_path")]
    pub assets: PathBuf,

    /// Extraction configuration
    #[serde(default)]
    pub config: ExtractionConfig,

    /// Sync configuration
    #[serde(default)]
    pub sync: SyncConfig,

    /// Custom directory-to-DataModel mapping
    /// Keys are DataModel paths (e.g., "ServerScriptService"), values are filesystem paths (e.g., "server")
    #[serde(default)]
    pub tree_mapping: HashMap<String, String>,

    /// License information (for commercial features)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<LicenseConfig>,

    /// Wally package configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<PackageConfig>,
}

fn default_tree_path() -> PathBuf {
    PathBuf::from("./src")
}

fn default_assets_path() -> PathBuf {
    PathBuf::from("./assets")
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "MyGame".to_string(),
            tree: default_tree_path(),
            assets: default_assets_path(),
            config: ExtractionConfig::default(),
            sync: SyncConfig::default(),
            tree_mapping: HashMap::new(),
            license: None,
            packages: None,
        }
    }
}

/// Configuration for game extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionConfig {
    /// Whether to extract binary assets (meshes, images, sounds)
    #[serde(default = "default_true")]
    pub extract_binary_assets: bool,

    /// Types of binary assets to extract
    #[serde(default = "default_binary_asset_types")]
    pub binary_asset_types: HashSet<String>,

    /// Services to exclude from extraction
    #[serde(default = "default_exclude_services")]
    pub exclude_services: HashSet<String>,

    /// Classes to exclude from extraction
    #[serde(default)]
    pub exclude_classes: HashSet<String>,

    /// How to handle script source code
    #[serde(default)]
    pub script_source_mode: ScriptSourceMode,

    /// How to handle terrain data
    #[serde(default)]
    pub terrain_mode: TerrainMode,

    /// How to handle CSG/Union operations
    #[serde(default)]
    pub csg_mode: CsgMode,

    /// Maximum instances per extraction chunk (for memory management)
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
}

fn default_true() -> bool {
    true
}

fn default_binary_asset_types() -> HashSet<String> {
    ["Mesh", "Image", "Sound", "Animation"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_exclude_services() -> HashSet<String> {
    ["CoreGui", "CorePackages", "RobloxPluginGuiService"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_chunk_size() -> usize {
    1000
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            extract_binary_assets: true,
            binary_asset_types: default_binary_asset_types(),
            exclude_services: default_exclude_services(),
            exclude_classes: HashSet::new(),
            script_source_mode: ScriptSourceMode::default(),
            terrain_mode: TerrainMode::default(),
            csg_mode: CsgMode::default(),
            chunk_size: default_chunk_size(),
        }
    }
}

/// How to store script source code
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ScriptSourceMode {
    /// Store source in external .luau files (recommended for git)
    #[default]
    External,

    /// Store source inline in the .rbxjson file
    Inline,
}

/// How to handle terrain data
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TerrainMode {
    /// Extract full voxel data to binary chunks
    #[default]
    VoxelData,

    /// Only store terrain properties, not voxel data
    PropertiesOnly,

    /// Skip terrain entirely
    Skip,
}

/// How to handle CSG/Union operations
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CsgMode {
    /// Store asset ID reference (requires network on sync)
    #[default]
    AssetReference,

    /// Extract mesh data locally
    LocalMesh,

    /// Skip CSG operations
    Skip,
}

/// Sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    /// Sync mode
    #[serde(default)]
    pub mode: SyncMode,

    /// How to handle conflicts
    #[serde(default)]
    pub conflict_resolution: ConflictResolution,

    /// Enable automatic sync when files change
    #[serde(default)]
    pub auto_sync: bool,

    /// Paths to watch for changes (relative to project root)
    #[serde(default)]
    pub watch_paths: Vec<PathBuf>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            mode: SyncMode::default(),
            conflict_resolution: ConflictResolution::default(),
            auto_sync: false,
            watch_paths: vec![PathBuf::from("./src")],
        }
    }
}

/// Sync direction mode
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SyncMode {
    /// Only push from files to Studio
    Push,

    /// Only pull from Studio to files
    Pull,

    /// Both directions
    #[default]
    Bidirectional,
}

/// How to handle sync conflicts
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictResolution {
    /// Prompt user to choose
    #[default]
    Prompt,

    /// Local files win
    KeepLocal,

    /// Studio version wins
    KeepRemote,

    /// Try to merge automatically
    AutoMerge,
}

/// License configuration for commercial features
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseConfig {
    /// License key
    pub key: String,

    /// Associated email
    pub email: String,
}

/// Wally package configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageConfig {
    /// Enable Wally package support
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Path to shared packages (relative to tree, default: "ReplicatedStorage/Packages")
    #[serde(default = "default_shared_packages_path")]
    pub shared_packages_path: String,

    /// Path to server packages (relative to tree, default: "ServerScriptService/Packages")
    #[serde(default = "default_server_packages_path")]
    pub server_packages_path: String,

    /// Exclude packages from file watcher (don't sync changes FROM packages)
    #[serde(default = "default_true")]
    pub exclude_from_watch: bool,

    /// Preserve packages during extraction (don't overwrite local package files)
    #[serde(default = "default_true")]
    pub preserve_on_extract: bool,

    /// Path to wally.toml (relative to project root, auto-detected if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wally_toml_path: Option<PathBuf>,

    /// Path to Packages folder on filesystem (default: "Packages")
    #[serde(default = "default_packages_folder")]
    pub packages_folder: PathBuf,
}

fn default_shared_packages_path() -> String {
    "ReplicatedStorage/Packages".to_string()
}

fn default_server_packages_path() -> String {
    "ServerScriptService/Packages".to_string()
}

fn default_packages_folder() -> PathBuf {
    PathBuf::from("Packages")
}

impl Default for PackageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            shared_packages_path: default_shared_packages_path(),
            server_packages_path: default_server_packages_path(),
            exclude_from_watch: true,
            preserve_on_extract: true,
            wally_toml_path: None,
            packages_folder: default_packages_folder(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ProjectConfig::default();
        assert_eq!(config.name, "MyGame");
        assert_eq!(config.tree, PathBuf::from("./src"));
        assert!(config.config.extract_binary_assets);
    }

    #[test]
    fn test_config_serialization() {
        let config = ProjectConfig {
            name: "TestGame".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        println!("{}", json);

        let deserialized: ProjectConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.name, deserialized.name);
    }
}
