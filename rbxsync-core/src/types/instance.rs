//! Roblox Instance representation
//!
//! An Instance is the fundamental unit in Roblox's DataModel.
//! This module defines how we serialize instances to JSON.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use super::{AttributeValue, PropertyValue};

/// A serialized Roblox instance
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    /// The Roblox class name (e.g., "Part", "Script", "Folder")
    pub class_name: String,

    /// The instance name
    pub name: String,

    /// Unique identifier for cross-references
    pub reference_id: Uuid,

    /// Instance properties (excluding Name which is stored separately)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, PropertyValue>,

    /// Instance attributes (custom user-defined values)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, AttributeValue>,

    /// CollectionService tags
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Child instances (when stored inline)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Instance>,

    /// For scripts: external source file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
}

impl Instance {
    /// Create a new instance with the given class name and name
    pub fn new(class_name: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            class_name: class_name.into(),
            name: name.into(),
            reference_id: Uuid::new_v4(),
            properties: BTreeMap::new(),
            attributes: BTreeMap::new(),
            tags: Vec::new(),
            children: Vec::new(),
            source_file: None,
        }
    }

    /// Set a property value
    pub fn set_property(&mut self, name: impl Into<String>, value: PropertyValue) {
        self.properties.insert(name.into(), value);
    }

    /// Set an attribute value
    pub fn set_attribute(&mut self, name: impl Into<String>, value: AttributeValue) {
        self.attributes.insert(name.into(), value);
    }

    /// Add a tag
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.tags.push(tag.into());
    }

    /// Add a child instance
    pub fn add_child(&mut self, child: Instance) {
        self.children.push(child);
    }

    /// Check if this is a script type
    pub fn is_script(&self) -> bool {
        matches!(
            self.class_name.as_str(),
            "Script" | "LocalScript" | "ModuleScript"
        )
    }

    /// Check if this is a service (top-level container)
    pub fn is_service(&self) -> bool {
        matches!(
            self.class_name.as_str(),
            "Workspace"
                | "ReplicatedStorage"
                | "ReplicatedFirst"
                | "ServerScriptService"
                | "ServerStorage"
                | "StarterGui"
                | "StarterPack"
                | "StarterPlayer"
                | "Lighting"
                | "SoundService"
                | "Chat"
                | "LocalizationService"
                | "TestService"
                | "HttpService"
                | "Teams"
                | "TextChatService"
        )
    }

    /// Get the appropriate file extension for scripts
    pub fn script_extension(&self) -> Option<&'static str> {
        match self.class_name.as_str() {
            "Script" => Some(".server.luau"),
            "LocalScript" => Some(".client.luau"),
            "ModuleScript" => Some(".luau"),
            _ => None,
        }
    }
}

/// Metadata for an instance stored in `_meta.rbxjson` files
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceMeta {
    /// The Roblox class name
    pub class_name: String,

    /// The instance name
    pub name: String,

    /// Unique identifier
    pub reference_id: Uuid,

    /// Properties for this instance
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, PropertyValue>,

    /// Attributes
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, AttributeValue>,

    /// Tags
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl From<&Instance> for InstanceMeta {
    fn from(instance: &Instance) -> Self {
        Self {
            class_name: instance.class_name.clone(),
            name: instance.name.clone(),
            reference_id: instance.reference_id,
            properties: instance.properties.clone(),
            attributes: instance.attributes.clone(),
            tags: instance.tags.clone(),
        }
    }
}

/// Terrain-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainData {
    /// Size of each voxel chunk
    pub chunk_size: u32,

    /// Voxel resolution (typically 4)
    pub resolution: u32,

    /// Bounding region of terrain data
    pub region: Region,

    /// Directory containing chunk files
    pub chunks_dir: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Region {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

/// CSG/Union operation data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CsgData {
    /// Cloud asset ID if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,

    /// Local mesh file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_mesh: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Color3, PropertyValue, Vector3};

    #[test]
    fn test_instance_creation() {
        let mut part = Instance::new("Part", "MyPart");
        part.set_property("Anchored", PropertyValue::Bool(true));
        part.set_property(
            "Size",
            PropertyValue::Vector3(Vector3 {
                x: 10.0,
                y: 1.0,
                z: 10.0,
            }),
        );
        part.set_property(
            "Color",
            PropertyValue::Color3(Color3 {
                r: 1.0,
                g: 0.0,
                b: 0.0,
            }),
        );

        let json = serde_json::to_string_pretty(&part).unwrap();
        println!("{}", json);

        // Verify round-trip
        let deserialized: Instance = serde_json::from_str(&json).unwrap();
        assert_eq!(part.class_name, deserialized.class_name);
        assert_eq!(part.name, deserialized.name);
        assert_eq!(part.properties.len(), deserialized.properties.len());
    }

    #[test]
    fn test_script_detection() {
        assert!(Instance::new("Script", "Main").is_script());
        assert!(Instance::new("LocalScript", "Client").is_script());
        assert!(Instance::new("ModuleScript", "Utils").is_script());
        assert!(!Instance::new("Part", "Block").is_script());
    }

    #[test]
    fn test_script_extensions() {
        assert_eq!(
            Instance::new("Script", "Main").script_extension(),
            Some(".server.luau")
        );
        assert_eq!(
            Instance::new("LocalScript", "Client").script_extension(),
            Some(".client.luau")
        );
        assert_eq!(
            Instance::new("ModuleScript", "Utils").script_extension(),
            Some(".luau")
        );
    }

    #[test]
    fn test_nested_tree_rbxjson_round_trip() {
        use crate::types::AttributeValue;

        let mut root = Instance::new("Model", "Root");
        root.set_property("Anchored", PropertyValue::Bool(true));
        root.set_attribute("Version", AttributeValue::Number(2.0));
        root.set_attribute("Owner", AttributeValue::String("ben".to_string()));
        root.add_tag("Spawn");

        let mut child = Instance::new("Part", "Child");
        child.set_property(
            "Size",
            PropertyValue::Vector3(Vector3 { x: 1.0, y: 2.0, z: 3.0 }),
        );

        let mut runner = Instance::new("Script", "Runner");
        runner.source_file = Some("Runner.server.luau".to_string());
        runner.set_property("Source", PropertyValue::String("print('run')".to_string()));

        child.add_child(runner);
        root.add_child(child);

        let json = serde_json::to_string_pretty(&root).unwrap();
        let back: Instance = serde_json::from_str(&json).unwrap();
        let rejson = serde_json::to_string_pretty(&back).unwrap();

        // Serialization is stable across a round trip
        assert_eq!(json, rejson);

        // Structure survives
        assert_eq!(back.class_name, "Model");
        assert_eq!(back.tags, vec!["Spawn".to_string()]);
        assert_eq!(back.attributes.len(), 2);
        assert_eq!(back.children.len(), 1);
        let child_back = &back.children[0];
        assert_eq!(child_back.class_name, "Part");
        assert_eq!(child_back.children.len(), 1);
        let runner_back = &child_back.children[0];
        assert_eq!(runner_back.class_name, "Script");
        assert_eq!(runner_back.source_file.as_deref(), Some("Runner.server.luau"));
        assert!(runner_back.is_script());
    }
}
