//! Wally package manager types
//!
//! Provides parsing for wally.toml manifests and wally.lock files
//! to enable Wally package compatibility in RbxSync.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Wally manifest (wally.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallyManifest {
    /// Package information
    pub package: WallyPackageInfo,

    /// Package dependencies (shared)
    #[serde(default)]
    pub dependencies: HashMap<String, String>,

    /// Server-only dependencies
    #[serde(default, rename = "server-dependencies")]
    pub server_dependencies: HashMap<String, String>,

    /// Dev dependencies (testing, etc.)
    #[serde(default, rename = "dev-dependencies")]
    pub dev_dependencies: HashMap<String, String>,
}

/// Package metadata in wally.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallyPackageInfo {
    /// Package name (scope/name format)
    pub name: String,

    /// Package version (semver)
    pub version: String,

    /// Wally registry URL
    #[serde(default = "default_registry")]
    pub registry: String,

    /// Package realm (shared, server, dev)
    #[serde(default = "default_realm")]
    pub realm: String,

    /// Package description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Package license
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Package authors
    #[serde(default)]
    pub authors: Vec<String>,
}

fn default_registry() -> String {
    "https://github.com/UpliftGames/wally-index".to_string()
}

fn default_realm() -> String {
    "shared".to_string()
}

/// Wally lock file (wally.lock)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallyLock {
    /// Registry URL
    pub registry: String,

    /// Resolved packages
    #[serde(default, rename = "package")]
    pub packages: Vec<WallyLockedPackage>,
}

/// A resolved/locked package in wally.lock
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallyLockedPackage {
    /// Package name (scope/name)
    pub name: String,

    /// Resolved version
    pub version: String,

    /// Package checksum
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,

    /// Dependencies of this package
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Errors that can occur when parsing Wally files
#[derive(Debug, thiserror::Error)]
pub enum WallyError {
    #[error("Failed to read file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse TOML: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Wally manifest not found at {0}")]
    ManifestNotFound(String),

    #[error("Wally lock file not found at {0}")]
    LockNotFound(String),
}

impl WallyManifest {
    /// Parse a wally.toml file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, WallyError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(WallyError::ManifestNotFound(path.display().to_string()));
        }
        let content = fs::read_to_string(path)?;
        let manifest: WallyManifest = toml::from_str(&content)?;
        Ok(manifest)
    }

    /// Get all dependencies across all realms
    pub fn all_dependencies(&self) -> HashMap<String, String> {
        let mut all = self.dependencies.clone();
        all.extend(self.server_dependencies.clone());
        all.extend(self.dev_dependencies.clone());
        all
    }

    /// Get package names that should go to ReplicatedStorage
    pub fn shared_packages(&self) -> Vec<String> {
        self.dependencies.keys().cloned().collect()
    }

    /// Get package names that should go to ServerScriptService/ServerStorage
    pub fn server_packages(&self) -> Vec<String> {
        self.server_dependencies.keys().cloned().collect()
    }
}

impl WallyLock {
    /// Parse a wally.lock file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, WallyError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(WallyError::LockNotFound(path.display().to_string()));
        }
        let content = fs::read_to_string(path)?;
        let lock: WallyLock = toml::from_str(&content)?;
        Ok(lock)
    }

    /// Find a locked package by name
    pub fn find_package(&self, name: &str) -> Option<&WallyLockedPackage> {
        self.packages.iter().find(|p| p.name == name)
    }

    /// Get all package names
    pub fn package_names(&self) -> Vec<&str> {
        self.packages.iter().map(|p| p.name.as_str()).collect()
    }
}

/// Find wally.toml in a project directory (checks root and common locations)
pub fn find_wally_manifest<P: AsRef<Path>>(project_dir: P) -> Option<std::path::PathBuf> {
    let project_dir = project_dir.as_ref();

    // Check common locations
    let candidates = [
        project_dir.join("wally.toml"),
        project_dir.join("src/wally.toml"),
    ];

    candidates.into_iter().find(|c| c.exists())
}

/// Find wally.lock in a project directory
pub fn find_wally_lock<P: AsRef<Path>>(project_dir: P) -> Option<std::path::PathBuf> {
    let project_dir = project_dir.as_ref();

    let candidates = [
        project_dir.join("wally.lock"),
        project_dir.join("src/wally.lock"),
    ];

    candidates.into_iter().find(|c| c.exists())
}

/// Determine if a path is within a Packages directory
pub fn is_package_path<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    let path_str = path.to_string_lossy().to_lowercase();

    // Check for common Wally package directory patterns
    path_str.contains("/packages/")
        || path_str.contains("\\packages\\")
        || path_str.ends_with("/packages")
        || path_str.ends_with("\\packages")
}

/// Get the standard Packages directory paths for different realms
#[derive(Debug, Clone)]
pub struct PackageDirectories {
    /// Shared packages (ReplicatedStorage/Packages)
    pub shared: String,
    /// Server packages (ServerScriptService/Packages or ServerStorage/Packages)
    pub server: String,
    /// Dev packages (ReplicatedStorage/DevPackages or similar)
    pub dev: String,
}

impl Default for PackageDirectories {
    fn default() -> Self {
        Self {
            shared: "ReplicatedStorage/Packages".to_string(),
            server: "ServerScriptService/Packages".to_string(),
            dev: "ReplicatedStorage/DevPackages".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_package_path() {
        assert!(is_package_path("src/ReplicatedStorage/Packages/signal"));
        assert!(is_package_path("ReplicatedStorage/Packages"));
        assert!(!is_package_path("src/ReplicatedStorage/Shared"));
        assert!(!is_package_path("src/ServerScriptService/Services"));
    }

    #[test]
    fn test_parse_wally_manifest() {
        let toml_content = r#"
[package]
name = "test/game"
version = "0.1.0"
registry = "https://github.com/UpliftGames/wally-index"
realm = "shared"

[dependencies]
Signal = "sleitnick/signal@1.5.0"
Promise = "evaera/promise@4.0.0"

[server-dependencies]
ProfileService = "madstudioroblox/profileservice@1.0.0"
"#;
        let manifest: WallyManifest = toml::from_str(toml_content).unwrap();
        assert_eq!(manifest.package.name, "test/game");
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.server_dependencies.len(), 1);
    }

    #[test]
    fn test_parse_wally_lock() {
        let toml_content = r#"
registry = "https://github.com/UpliftGames/wally-index"

[[package]]
name = "sleitnick/signal"
version = "1.5.0"
checksum = "abc123"
dependencies = []

[[package]]
name = "evaera/promise"
version = "4.0.0"
dependencies = ["sleitnick/signal"]
"#;
        let lock: WallyLock = toml::from_str(toml_content).unwrap();
        assert_eq!(lock.packages.len(), 2);
        assert!(lock.find_package("sleitnick/signal").is_some());
    }
}
