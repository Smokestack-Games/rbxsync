//! RbxSync Core Library
//!
//! This crate provides the core functionality for RbxSync:
//! - Roblox property type definitions and serialization
//! - Instance representation
//! - Project configuration
//! - Plugin building (.rbxm generation)
//! - Rojo project file parsing and migration
//! - Luau obfuscation for build-time transforms

pub mod obfuscator;
pub mod path_utils;
pub mod plugin_builder;
pub mod rojo;
pub mod types;

// Re-export commonly used types
pub use obfuscator::{Obfuscator, ObfuscatorConfig, ObfuscationResult};
pub use plugin_builder::{build_plugin, build_plugin_with_stats, get_studio_plugins_folder, install_plugin, PluginBuildConfig, PluginBuildStats};
pub use rojo::{
    find_rojo_project, parse_rojo_project, rojo_to_tree_mapping, RojoError, RojoProject, RojoTree,
};
pub use types::{
    AttributeValue, CFrame, Color3, EnumValue, Instance, InstanceMeta, ProjectConfig,
    PropertyValue, Vector2, Vector3,
    // Wally package support
    PackageConfig, PackageDirectories, WallyError, WallyLock, WallyLockedPackage,
    WallyManifest, WallyPackageInfo, find_wally_manifest, find_wally_lock, is_package_path,
    // Harness system for multi-session AI development
    Feature, FeaturePriority, FeatureStatus, FeaturesFile, GameDefinition,
    HarnessState, SessionLog, SessionLogEntry,
};
pub use path_utils::{normalize_path, path_to_string, path_with_suffix, pathbuf_with_suffix, sanitize_filename};
