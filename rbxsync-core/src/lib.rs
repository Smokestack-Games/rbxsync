//! RbxSync Core Library
//!
//! This crate provides the core functionality for RbxSync:
//! - Roblox property type definitions and serialization
//! - Instance representation
//! - Project configuration
//! - Plugin building (.rbxm generation)
//! - Rojo project file parsing and migration

pub mod plugin_builder;
pub mod rojo;
pub mod types;

// Re-export commonly used types
pub use plugin_builder::{build_plugin, get_studio_plugins_folder, install_plugin, PluginBuildConfig};
pub use rojo::{
    find_rojo_project, parse_rojo_project, rojo_to_tree_mapping, RojoError, RojoProject, RojoTree,
};
pub use types::{
    AttributeValue, CFrame, Color3, EnumValue, Instance, InstanceMeta, ProjectConfig,
    PropertyValue, Vector2, Vector3,
};
