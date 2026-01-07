//! RbxSync Core Library
//!
//! This crate provides the core functionality for RbxSync:
//! - Roblox property type definitions and serialization
//! - Instance representation
//! - Project configuration
//! - Plugin building (.rbxm generation)

pub mod plugin_builder;
pub mod types;

// Re-export commonly used types
pub use plugin_builder::{build_plugin, get_studio_plugins_folder, install_plugin, PluginBuildConfig};
pub use types::{
    AttributeValue, CFrame, Color3, EnumValue, Instance, InstanceMeta, ProjectConfig,
    PropertyValue, Vector2, Vector3,
};
