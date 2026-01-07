//! Plugin Builder
//!
//! Bundles Luau source files into a Roblox plugin .rbxm file using rbx-dom.

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rbx_dom_weak::types::Variant;
use rbx_dom_weak::{InstanceBuilder, WeakDom};

/// Configuration for building a plugin
#[derive(Debug, Clone)]
pub struct PluginBuildConfig {
    /// Directory containing the plugin source files
    pub source_dir: PathBuf,
    /// Output path for the .rbxm file
    pub output_path: PathBuf,
    /// Name of the plugin (used as root instance name)
    pub plugin_name: String,
}

impl Default for PluginBuildConfig {
    fn default() -> Self {
        Self {
            source_dir: PathBuf::from("plugin/src"),
            output_path: PathBuf::from("build/RbxSync.rbxm"),
            plugin_name: "RbxSync".to_string(),
        }
    }
}

/// Represents a Luau script file
#[derive(Debug)]
struct ScriptFile {
    /// File name without extension
    name: String,
    /// Script class (Script, LocalScript, ModuleScript)
    class_name: String,
    /// Source code content
    source: String,
    /// Whether this is the entry point (init.server.luau)
    is_entry: bool,
}

/// Build a Roblox plugin .rbxm from Luau source files
pub fn build_plugin(config: &PluginBuildConfig) -> Result<PathBuf> {
    // Ensure source directory exists
    if !config.source_dir.exists() {
        anyhow::bail!(
            "Plugin source directory not found: {}",
            config.source_dir.display()
        );
    }

    // Collect all script files
    let scripts = collect_scripts(&config.source_dir)?;

    if scripts.is_empty() {
        anyhow::bail!("No .luau files found in {}", config.source_dir.display());
    }

    // Find the entry point (init.server.luau)
    let entry_script = scripts
        .iter()
        .find(|s| s.is_entry)
        .context("No entry point found. Expected init.server.luau")?;

    // Build the instance tree
    let dom = build_dom(&config.plugin_name, entry_script, &scripts)?;

    // Ensure output directory exists
    if let Some(parent) = config.output_path.parent() {
        fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    // Write to .rbxm file
    let output_file =
        BufWriter::new(File::create(&config.output_path).context("Failed to create output file")?);

    let root_refs = vec![dom.root_ref()];
    rbx_binary::to_writer(output_file, &dom, &root_refs).context("Failed to write .rbxm file")?;

    Ok(config.output_path.clone())
}

/// Collect all Luau script files from the source directory
fn collect_scripts(source_dir: &Path) -> Result<Vec<ScriptFile>> {
    let mut scripts = Vec::new();

    for entry in fs::read_dir(source_dir).context("Failed to read source directory")? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "luau" || ext == "lua" {
                    let script = parse_script_file(&path)?;
                    scripts.push(script);
                }
            }
        }
    }

    Ok(scripts)
}

/// Parse a Luau file and determine its script type
fn parse_script_file(path: &Path) -> Result<ScriptFile> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid file name")?;

    let source = fs::read_to_string(path).context("Failed to read script file")?;

    // Determine script type from filename
    let (name, class_name, is_entry) = if file_name == "init.server.luau"
        || file_name == "init.server.lua"
    {
        // Entry point - will become the root Script
        ("init".to_string(), "Script".to_string(), true)
    } else if file_name.ends_with(".server.luau") || file_name.ends_with(".server.lua") {
        let name = file_name
            .trim_end_matches(".server.luau")
            .trim_end_matches(".server.lua")
            .to_string();
        (name, "Script".to_string(), false)
    } else if file_name.ends_with(".client.luau") || file_name.ends_with(".client.lua") {
        let name = file_name
            .trim_end_matches(".client.luau")
            .trim_end_matches(".client.lua")
            .to_string();
        (name, "LocalScript".to_string(), false)
    } else {
        // Regular .luau file -> ModuleScript
        let name = file_name
            .trim_end_matches(".luau")
            .trim_end_matches(".lua")
            .to_string();
        (name, "ModuleScript".to_string(), false)
    };

    Ok(ScriptFile {
        name,
        class_name,
        source,
        is_entry,
    })
}

/// Build the WeakDom instance tree
fn build_dom(plugin_name: &str, entry: &ScriptFile, all_scripts: &[ScriptFile]) -> Result<WeakDom> {
    // Create root Script instance (the entry point)
    let mut root_builder = InstanceBuilder::new(&entry.class_name)
        .with_name(plugin_name)
        .with_property("Source", Variant::String(entry.source.clone()));

    // Add all non-entry scripts as children
    for script in all_scripts {
        if script.is_entry {
            continue;
        }

        let child = InstanceBuilder::new(&script.class_name)
            .with_name(&script.name)
            .with_property("Source", Variant::String(script.source.clone()));

        root_builder = root_builder.with_child(child);
    }

    Ok(WeakDom::new(root_builder))
}

/// Get the default Studio plugins folder path for the current platform
pub fn get_studio_plugins_folder() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|home| {
            home.join("Documents")
                .join("Roblox")
                .join("Plugins")
        })
    }

    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir().map(|local| {
            local
                .join("Roblox")
                .join("Plugins")
        })
    }

    #[cfg(target_os = "linux")]
    {
        // Linux typically uses Wine/Proton, plugins location varies
        dirs::home_dir().map(|home| {
            home.join(".local")
                .join("share")
                .join("roblox")
                .join("plugins")
        })
    }
}

/// Install a plugin to Studio's plugins folder
pub fn install_plugin(rbxm_path: &Path, plugin_name: &str) -> Result<PathBuf> {
    let plugins_folder =
        get_studio_plugins_folder().context("Could not determine Studio plugins folder")?;

    // Create plugins folder if it doesn't exist
    fs::create_dir_all(&plugins_folder).context("Failed to create plugins folder")?;

    let dest_path = plugins_folder.join(format!("{}.rbxm", plugin_name));

    fs::copy(rbxm_path, &dest_path).context("Failed to copy plugin to Studio folder")?;

    Ok(dest_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_script_file_module() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("MyModule.luau");
        fs::write(&path, "return {}").unwrap();

        let script = parse_script_file(&path).unwrap();
        assert_eq!(script.name, "MyModule");
        assert_eq!(script.class_name, "ModuleScript");
        assert!(!script.is_entry);
    }

    #[test]
    fn test_parse_script_file_server() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("Main.server.luau");
        fs::write(&path, "print('hello')").unwrap();

        let script = parse_script_file(&path).unwrap();
        assert_eq!(script.name, "Main");
        assert_eq!(script.class_name, "Script");
        assert!(!script.is_entry);
    }

    #[test]
    fn test_parse_script_file_entry() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("init.server.luau");
        fs::write(&path, "-- entry").unwrap();

        let script = parse_script_file(&path).unwrap();
        assert_eq!(script.name, "init");
        assert_eq!(script.class_name, "Script");
        assert!(script.is_entry);
    }

    #[test]
    fn test_build_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        // Create test files
        fs::write(src_dir.join("init.server.luau"), "-- entry point").unwrap();
        fs::write(src_dir.join("Helper.luau"), "return {}").unwrap();

        let output_path = temp_dir.path().join("output.rbxm");

        let config = PluginBuildConfig {
            source_dir: src_dir,
            output_path: output_path.clone(),
            plugin_name: "TestPlugin".to_string(),
        };

        let result = build_plugin(&config).unwrap();
        assert!(result.exists());
        assert!(result.metadata().unwrap().len() > 0);
    }
}
