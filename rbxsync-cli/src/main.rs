//! RbxSync CLI
//!
//! Command-line interface for Roblox game extraction and synchronization.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rbx_dom_weak::types::Variant;
use rbx_dom_weak::{InstanceBuilder, WeakDom};
use rbxsync_core::{build_plugin, get_studio_plugins_folder, install_plugin, PluginBuildConfig, ProjectConfig};
use rbxsync_server::{run_server, ServerConfig};

#[derive(Parser)]
#[command(name = "rbxsync")]
#[command(about = "Roblox game extraction and synchronization tool")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new RbxSync project
    Init {
        /// Project name
        #[arg(short, long)]
        name: Option<String>,

        /// Directory to initialize (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// Launch Roblox Studio
    Studio {
        /// Place file to open (.rbxl or .rbxlx)
        place: Option<PathBuf>,

        /// Start sync server in background
        #[arg(short, long)]
        serve: bool,
    },

    /// Start or stop playtest in connected Studio
    Debug {
        #[command(subcommand)]
        action: DebugAction,
    },

    /// Extract game from connected Roblox Studio
    Extract {
        /// Specific services to extract (default: all)
        #[arg(short, long)]
        service: Option<Vec<String>>,

        /// Include terrain data
        #[arg(long, default_value = "true")]
        terrain: bool,

        /// Include binary assets
        #[arg(long, default_value = "true")]
        assets: bool,

        /// Output directory (default: project src directory)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Start the sync server (connects to Studio plugin)
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "44755")]
        port: u16,
    },

    /// Stop the running sync server
    Stop,

    /// Show sync status
    Status,

    /// Show diff between local files and Studio
    Diff,

    /// Sync local changes to connected Studio instance
    Sync {
        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// Build the Studio plugin as .rbxm file
    BuildPlugin {
        /// Source directory containing Luau files (default: plugin/src)
        #[arg(short, long)]
        source: Option<PathBuf>,

        /// Output path for the .rbxm file (default: build/RbxSync.rbxm)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Plugin name (default: RbxSync)
        #[arg(short, long)]
        name: Option<String>,

        /// Install plugin to Studio's plugins folder after building
        #[arg(long)]
        install: bool,
    },

    /// Manage the RbxSync Studio plugin
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },

    /// Generate sourcemap.json for Luau LSP
    Sourcemap {
        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Output file (default: sourcemap.json)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include non-script instances
        #[arg(long, default_value = "false")]
        include_non_scripts: bool,
    },

    /// Build a .rbxl or .rbxm file from project files
    Build {
        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Output file (default: build/game.rbxl)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format: rbxl (place) or rbxm (model)
        #[arg(short, long, default_value = "rbxl")]
        format: String,
    },
}

#[derive(Subcommand)]
enum PluginAction {
    /// Install the plugin to Roblox Studio's plugins folder
    Install {
        /// Path to .rbxm plugin file (default: build/RbxSync.rbxm)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Plugin name (default: RbxSync)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Uninstall the plugin from Roblox Studio's plugins folder
    Uninstall {
        /// Plugin name to uninstall (default: RbxSync)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// List installed Roblox Studio plugins
    List,
}

#[derive(Subcommand)]
enum DebugAction {
    /// Start a playtest (Run mode)
    Start {
        /// Playtest mode: run (default), play, server
        #[arg(short, long, default_value = "run")]
        mode: String,
    },
    /// Stop the current playtest
    Stop,
    /// Show playtest status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rbxsync=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name, path } => {
            cmd_init(name, path).await?;
        }
        Commands::Studio { place, serve } => {
            cmd_studio(place, serve).await?;
        }
        Commands::Debug { action } => {
            cmd_debug(action).await?;
        }
        Commands::Extract {
            service,
            terrain,
            assets,
            output,
        } => {
            cmd_extract(service, terrain, assets, output).await?;
        }
        Commands::Serve { port } => {
            cmd_serve(port).await?;
        }
        Commands::Stop => {
            cmd_stop().await?;
        }
        Commands::Status => {
            cmd_status().await?;
        }
        Commands::Diff => {
            cmd_diff().await?;
        }
        Commands::Sync { path } => {
            cmd_sync(path).await?;
        }
        Commands::BuildPlugin {
            source,
            output,
            name,
            install,
        } => {
            cmd_build_plugin(source, output, name, install)?;
        }
        Commands::Plugin { action } => {
            cmd_plugin(action)?;
        }
        Commands::Sourcemap {
            path,
            output,
            include_non_scripts,
        } => {
            cmd_sourcemap(path, output, include_non_scripts)?;
        }
        Commands::Build {
            path,
            output,
            format,
        } => {
            cmd_build(path, output, format)?;
        }
    }

    Ok(())
}

/// Initialize a new project
async fn cmd_init(name: Option<String>, path: Option<PathBuf>) -> Result<()> {
    let project_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
    let project_name = name.unwrap_or_else(|| {
        project_dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "MyGame".to_string())
    });

    tracing::info!("Initializing RbxSync project: {}", project_name);

    // Create directory structure
    let src_dir = project_dir.join("src");
    let assets_dir = project_dir.join("assets");
    let terrain_dir = project_dir.join("terrain");

    std::fs::create_dir_all(&src_dir).context("Failed to create src directory")?;
    std::fs::create_dir_all(&assets_dir).context("Failed to create assets directory")?;
    std::fs::create_dir_all(&terrain_dir).context("Failed to create terrain directory")?;

    // Create default service directories
    for service in &[
        "Workspace",
        "ReplicatedStorage",
        "ServerScriptService",
        "ServerStorage",
        "StarterGui",
        "StarterPack",
        "StarterPlayer",
    ] {
        std::fs::create_dir_all(src_dir.join(service))
            .context(format!("Failed to create {} directory", service))?;
    }

    // Create project config
    let config = ProjectConfig {
        name: project_name.clone(),
        ..Default::default()
    };

    let config_path = project_dir.join("rbxsync.json");
    let config_json = serde_json::to_string_pretty(&config)?;
    std::fs::write(&config_path, config_json).context("Failed to write rbxsync.json")?;

    // Create .gitignore
    let gitignore_path = project_dir.join(".gitignore");
    let gitignore_content = r#"# RbxSync
.rbxsync/
*.rbxl
*.rbxlx

# Binary assets (optional - uncomment to exclude)
# assets/

# OS files
.DS_Store
Thumbs.db
"#;
    std::fs::write(&gitignore_path, gitignore_content).context("Failed to write .gitignore")?;

    println!("Initialized RbxSync project '{}' at {:?}", project_name, project_dir);
    println!("\nProject structure:");
    println!("  rbxsync.json      - Project configuration");
    println!("  src/              - Instance tree");
    println!("  assets/           - Binary assets (meshes, images, sounds)");
    println!("  terrain/          - Terrain voxel data");
    println!("\nNext steps:");
    println!("  1. Open your game in Roblox Studio");
    println!("  2. Install the RbxSync plugin");
    println!("  3. Run: rbxsync extract");

    Ok(())
}

/// Launch Roblox Studio
async fn cmd_studio(place: Option<PathBuf>, serve: bool) -> Result<()> {
    // Find Roblox Studio installation
    let studio_path = find_studio_path()?;

    println!("Found Roblox Studio at: {}", studio_path.display());

    // Optionally start the sync server
    if serve {
        let client = reqwest::Client::new();
        if client.get("http://localhost:44755/health").send().await.is_err() {
            println!("Starting sync server in background...");
            let config = ServerConfig::default();
            tokio::spawn(async move {
                if let Err(e) = run_server(config).await {
                    tracing::error!("Server error: {}", e);
                }
            });
            // Give server time to start
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        } else {
            println!("Sync server already running.");
        }
    }

    // Build command to launch Studio
    let mut command = std::process::Command::new("open");

    #[cfg(target_os = "macos")]
    {
        command.arg("-a").arg(&studio_path);
        if let Some(ref place_file) = place {
            // Validate the file exists and has correct extension
            if !place_file.exists() {
                anyhow::bail!("Place file not found: {}", place_file.display());
            }
            let ext = place_file
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if ext != "rbxl" && ext != "rbxlx" {
                anyhow::bail!("Invalid place file format. Expected .rbxl or .rbxlx");
            }
            command.arg(place_file);
        }
    }

    #[cfg(target_os = "windows")]
    {
        command = std::process::Command::new(&studio_path);
        if let Some(ref place_file) = place {
            if !place_file.exists() {
                anyhow::bail!("Place file not found: {}", place_file.display());
            }
            command.arg(place_file);
        }
    }

    println!("Launching Roblox Studio...");
    command
        .spawn()
        .context("Failed to launch Roblox Studio")?;

    if let Some(place_file) = place {
        println!("Opening: {}", place_file.display());
    }

    if serve {
        println!("\nSync server is running. Press Ctrl+C to stop.");
        // Keep running to serve
        tokio::signal::ctrl_c().await?;
    }

    Ok(())
}

/// Control playtest in Studio
async fn cmd_debug(action: DebugAction) -> Result<()> {
    let client = reqwest::Client::new();

    // Check server is running
    if client.get("http://localhost:44755/health").send().await.is_err() {
        println!("RbxSync server is not running. Start it with: rbxsync serve");
        return Ok(());
    }

    match action {
        DebugAction::Start { mode } => {
            println!("Starting playtest (mode: {})...", mode);

            let response = client
                .post("http://localhost:44755/sync/command")
                .json(&serde_json::json!({
                    "command": "debug:start",
                    "payload": {
                        "mode": mode
                    }
                }))
                .send()
                .await
                .context("Failed to send debug start command")?;

            let result: serde_json::Value = response.json().await?;
            if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                println!("Playtest started.");
            } else {
                let error = result
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                println!("Failed to start playtest: {}", error);
            }
        }
        DebugAction::Stop => {
            println!("Stopping playtest...");

            let response = client
                .post("http://localhost:44755/sync/command")
                .json(&serde_json::json!({
                    "command": "debug:stop",
                    "payload": {}
                }))
                .send()
                .await
                .context("Failed to send debug stop command")?;

            let result: serde_json::Value = response.json().await?;
            if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                println!("Playtest stopped.");
            } else {
                let error = result
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                println!("Failed to stop playtest: {}", error);
            }
        }
        DebugAction::Status => {
            let response = client
                .post("http://localhost:44755/sync/command")
                .json(&serde_json::json!({
                    "command": "debug:status",
                    "payload": {}
                }))
                .send()
                .await
                .context("Failed to get debug status")?;

            let result: serde_json::Value = response.json().await?;
            if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                let data = result.get("data").cloned().unwrap_or_default();
                let running = data.get("running").and_then(|v| v.as_bool()).unwrap_or(false);
                let mode = data.get("mode").and_then(|v| v.as_str()).unwrap_or("unknown");

                if running {
                    println!("Playtest is running (mode: {})", mode);
                } else {
                    println!("No playtest running");
                }
            } else {
                let error = result
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                println!("Failed to get status: {}", error);
            }
        }
    }

    Ok(())
}

/// Find Roblox Studio installation path
fn find_studio_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let default_path = PathBuf::from("/Applications/RobloxStudio.app");
        if default_path.exists() {
            return Ok(default_path);
        }

        // Try user Applications folder
        if let Ok(home) = std::env::var("HOME") {
            let user_path = PathBuf::from(home).join("Applications/RobloxStudio.app");
            if user_path.exists() {
                return Ok(user_path);
            }
        }

        anyhow::bail!(
            "Roblox Studio not found. Expected at:\n  - /Applications/RobloxStudio.app\n  - ~/Applications/RobloxStudio.app"
        );
    }

    #[cfg(target_os = "windows")]
    {
        // Check common Windows install locations
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let program_files = std::env::var("PROGRAMFILES(X86)")
            .or_else(|_| std::env::var("PROGRAMFILES"))
            .unwrap_or_default();

        let possible_paths = [
            PathBuf::from(&local_app_data).join("Roblox/Versions"),
            PathBuf::from(&program_files).join("Roblox/Versions"),
        ];

        for versions_dir in possible_paths {
            if versions_dir.exists() {
                // Find the latest version with RobloxStudioBeta.exe
                if let Ok(entries) = std::fs::read_dir(&versions_dir) {
                    for entry in entries.flatten() {
                        let studio_exe = entry.path().join("RobloxStudioBeta.exe");
                        if studio_exe.exists() {
                            return Ok(studio_exe);
                        }
                    }
                }
            }
        }

        anyhow::bail!("Roblox Studio not found. Please install it from roblox.com");
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        anyhow::bail!("Roblox Studio is not available on this platform");
    }
}

/// Extract game from Studio
async fn cmd_extract(
    services: Option<Vec<String>>,
    terrain: bool,
    assets: bool,
    _output: Option<PathBuf>,
) -> Result<()> {
    tracing::info!("Starting extraction...");

    // Check if server is running
    let client = reqwest::Client::new();
    let health_check = client.get("http://localhost:44755/health").send().await;

    if health_check.is_err() {
        println!("RbxSync server is not running.");
        println!("Starting server in background...");

        // Start server in background
        tokio::spawn(async {
            if let Err(e) = run_server(ServerConfig::default()).await {
                tracing::error!("Server error: {}", e);
            }
        });

        // Wait for server to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // Send extraction request
    let response = client
        .post("http://localhost:44755/extract/start")
        .json(&serde_json::json!({
            "services": services,
            "includeTerrain": terrain,
            "includeAssets": assets,
        }))
        .send()
        .await
        .context("Failed to start extraction")?;

    let result: serde_json::Value = response.json().await?;
    println!("Extraction started: {}", serde_json::to_string_pretty(&result)?);

    println!("\nWaiting for Studio plugin to send data...");
    println!("Make sure the RbxSync plugin is enabled in Roblox Studio.");

    // Poll for completion
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let status = client
            .get("http://localhost:44755/extract/status")
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if let Some(complete) = status.get("complete").and_then(|v| v.as_bool()) {
            if complete {
                let chunks = status.get("chunksReceived").and_then(|v| v.as_u64()).unwrap_or(0);
                println!("\nExtraction complete! Received {} chunks.", chunks);
                break;
            }
        }

        if let Some(received) = status.get("chunksReceived").and_then(|v| v.as_u64()) {
            if let Some(total) = status.get("totalChunks").and_then(|v| v.as_u64()) {
                print!("\rReceived {}/{} chunks...", received, total);
            } else {
                print!("\rReceived {} chunks...", received);
            }
        }
    }

    Ok(())
}

/// Start the sync server
async fn cmd_serve(port: u16) -> Result<()> {
    println!("Starting RbxSync server on port {}...", port);
    println!("Stop with: rbxsync stop");
    run_server(ServerConfig {
        port,
        ..Default::default()
    })
    .await
}

/// Stop the running sync server
async fn cmd_stop() -> Result<()> {
    let client = reqwest::Client::new();

    match client.post("http://localhost:44755/shutdown").send().await {
        Ok(response) if response.status().is_success() => {
            println!("Server stopped.");
            Ok(())
        }
        Ok(_) => {
            // Try killing by port as fallback
            println!("Server did not respond to shutdown. Trying force stop...");
            #[cfg(unix)]
            {
                let output = std::process::Command::new("lsof")
                    .args(["-ti", ":44755"])
                    .output();
                if let Ok(output) = output {
                    let pids = String::from_utf8_lossy(&output.stdout);
                    for pid in pids.lines() {
                        if let Ok(pid) = pid.trim().parse::<i32>() {
                            unsafe {
                                libc::kill(pid, libc::SIGTERM);
                            }
                            println!("Killed process {}", pid);
                        }
                    }
                }
            }
            Ok(())
        }
        Err(_) => {
            // Server not responding - try force kill by port
            #[cfg(unix)]
            {
                let output = std::process::Command::new("lsof")
                    .args(["-ti", ":44755"])
                    .output();
                if let Ok(output) = output {
                    let pids = String::from_utf8_lossy(&output.stdout);
                    let pids: Vec<&str> = pids.lines().collect();
                    if !pids.is_empty() {
                        println!("Server not responding. Force stopping...");
                        for pid in pids {
                            if let Ok(pid) = pid.trim().parse::<i32>() {
                                unsafe {
                                    libc::kill(pid, libc::SIGKILL);
                                }
                                println!("Killed process {}", pid);
                            }
                        }
                        return Ok(());
                    }
                }
            }
            println!("Server is not running.");
            Ok(())
        }
    }
}

/// Show status
async fn cmd_status() -> Result<()> {
    let client = reqwest::Client::new();

    match client.get("http://localhost:44755/health").send().await {
        Ok(response) => {
            let health: serde_json::Value = response.json().await?;
            println!("Server status: {}", serde_json::to_string_pretty(&health)?);

            // Check extraction status
            let status = client
                .get("http://localhost:44755/extract/status")
                .send()
                .await?
                .json::<serde_json::Value>()
                .await?;

            println!("Extraction status: {}", serde_json::to_string_pretty(&status)?);
        }
        Err(_) => {
            println!("Server is not running.");
            println!("Start it with: rbxsync serve");
        }
    }

    Ok(())
}

/// Show diff
async fn cmd_diff() -> Result<()> {
    println!("Diff functionality not yet implemented.");
    println!("This will show differences between local files and Studio.");
    Ok(())
}

/// Sync local changes to Studio
async fn cmd_sync(path: Option<PathBuf>) -> Result<()> {
    let project_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
    let project_dir_str = project_dir.to_string_lossy().to_string();

    tracing::info!("Syncing from {:?}...", project_dir);

    let client = reqwest::Client::new();

    // Check server is running
    if client.get("http://localhost:44755/health").send().await.is_err() {
        println!("RbxSync server is not running. Start it with: rbxsync serve");
        return Ok(());
    }

    // Read the local tree
    println!("Reading local files...");
    let tree_response = client
        .post("http://localhost:44755/sync/read-tree")
        .json(&serde_json::json!({
            "project_dir": project_dir_str
        }))
        .send()
        .await
        .context("Failed to read local tree")?;

    let tree: serde_json::Value = tree_response.json().await?;
    let instances = tree.get("instances").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    if instances.is_empty() {
        println!("No changes to sync.");
        return Ok(());
    }

    println!("Found {} instances to sync", instances.len());

    // Build sync operations
    let operations: Vec<serde_json::Value> = instances
        .into_iter()
        .map(|inst| {
            serde_json::json!({
                "op": "update",
                "path": inst.get("path"),
                "class_name": inst.get("className"),
                "name": inst.get("name"),
                "properties": inst.get("properties"),
                "source": inst.get("source")
            })
        })
        .collect();

    // Send batch sync
    println!("Syncing to Studio...");
    let sync_response = client
        .post("http://localhost:44755/sync/batch")
        .json(&serde_json::json!({
            "operations": operations
        }))
        .send()
        .await
        .context("Failed to sync")?;

    let result: serde_json::Value = sync_response.json().await?;

    if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        let applied = result.get("applied").and_then(|v| v.as_u64()).unwrap_or(0);
        println!("Successfully synced {} instances to Studio.", applied);
    } else {
        let errors = result.get("errors").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        println!("Sync completed with errors:");
        for err in errors {
            println!("  - {}", err);
        }
    }

    Ok(())
}

/// Build the Studio plugin as .rbxm
fn cmd_build_plugin(
    source: Option<PathBuf>,
    output: Option<PathBuf>,
    name: Option<String>,
    install: bool,
) -> Result<()> {
    let config = PluginBuildConfig {
        source_dir: source.unwrap_or_else(|| PathBuf::from("plugin/src")),
        output_path: output.unwrap_or_else(|| PathBuf::from("build/RbxSync.rbxm")),
        plugin_name: name.unwrap_or_else(|| "RbxSync".to_string()),
    };

    println!("Building plugin from {:?}...", config.source_dir);

    let output_path = build_plugin(&config).context("Failed to build plugin")?;

    println!("Plugin built successfully: {}", output_path.display());

    if install {
        println!("Installing plugin to Studio...");
        let installed_path =
            install_plugin(&output_path, &config.plugin_name).context("Failed to install plugin")?;
        println!("Plugin installed to: {}", installed_path.display());
        println!("\nRestart Roblox Studio to load the plugin.");
    } else {
        println!("\nTo install, run: rbxsync build-plugin --install");
        println!("Or manually copy {} to your Studio plugins folder.", output_path.display());
    }

    Ok(())
}

/// Manage the Studio plugin
fn cmd_plugin(action: PluginAction) -> Result<()> {
    let plugins_folder = get_studio_plugins_folder()
        .context("Could not determine Studio plugins folder")?;

    match action {
        PluginAction::Install { path, name } => {
            let plugin_path = path.unwrap_or_else(|| PathBuf::from("build/RbxSync.rbxm"));
            let plugin_name = name.unwrap_or_else(|| "RbxSync".to_string());

            if !plugin_path.exists() {
                // Try to build if source exists
                if PathBuf::from("plugin/src").exists() {
                    println!("Plugin file not found. Building from source...");
                    let config = PluginBuildConfig {
                        source_dir: PathBuf::from("plugin/src"),
                        output_path: plugin_path.clone(),
                        plugin_name: plugin_name.clone(),
                    };
                    build_plugin(&config).context("Failed to build plugin")?;
                } else {
                    anyhow::bail!(
                        "Plugin file not found: {}\nBuild the plugin first with: rbxsync build-plugin",
                        plugin_path.display()
                    );
                }
            }

            println!("Installing plugin to Studio...");
            let installed_path =
                install_plugin(&plugin_path, &plugin_name).context("Failed to install plugin")?;
            println!("Plugin installed to: {}", installed_path.display());
            println!("\nRestart Roblox Studio to load the plugin.");
        }
        PluginAction::Uninstall { name } => {
            let plugin_name = name.unwrap_or_else(|| "RbxSync".to_string());
            let plugin_path = plugins_folder.join(format!("{}.rbxm", plugin_name));

            if !plugin_path.exists() {
                println!("Plugin '{}' is not installed.", plugin_name);
                return Ok(());
            }

            std::fs::remove_file(&plugin_path).context("Failed to remove plugin file")?;
            println!("Plugin '{}' uninstalled from: {}", plugin_name, plugin_path.display());
            println!("\nRestart Roblox Studio to apply changes.");
        }
        PluginAction::List => {
            println!("Studio plugins folder: {}", plugins_folder.display());
            println!();

            if !plugins_folder.exists() {
                println!("  (folder does not exist)");
                return Ok(());
            }

            let entries: Vec<_> = std::fs::read_dir(&plugins_folder)
                .context("Failed to read plugins folder")?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "rbxm" || ext == "rbxmx")
                        .unwrap_or(false)
                })
                .collect();

            if entries.is_empty() {
                println!("  No plugins installed.");
            } else {
                println!("Installed plugins:");
                for entry in entries {
                    let name = entry.file_name();
                    let metadata = entry.metadata().ok();
                    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

                    println!(
                        "  {} ({:.1} KB)",
                        name.to_string_lossy(),
                        size as f64 / 1024.0
                    );
                }
            }
        }
    }

    Ok(())
}

/// Generate sourcemap for Luau LSP
fn cmd_sourcemap(
    path: Option<PathBuf>,
    output: Option<PathBuf>,
    include_non_scripts: bool,
) -> Result<()> {
    let project_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
    let output_path = output.unwrap_or_else(|| project_dir.join("sourcemap.json"));
    let src_dir = project_dir.join("src");

    if !src_dir.exists() {
        anyhow::bail!("Source directory not found: {}", src_dir.display());
    }

    println!("Generating sourcemap from {:?}...", src_dir);

    // Build the sourcemap tree
    let root = build_sourcemap_node("game", "DataModel", &src_dir, include_non_scripts)?;

    // Write to file
    let json = serde_json::to_string_pretty(&root)?;
    std::fs::write(&output_path, json).context("Failed to write sourcemap")?;

    println!("Sourcemap written to: {}", output_path.display());
    println!("\nTo use with Luau LSP, add to .luaurc:");
    println!("{{");
    println!("  \"languageMode\": \"strict\",");
    println!("  \"aliases\": {{}}");
    println!("}}");

    Ok(())
}

/// Build a sourcemap node recursively
fn build_sourcemap_node(
    name: &str,
    class_name: &str,
    dir_path: &std::path::Path,
    include_non_scripts: bool,
) -> Result<serde_json::Value> {
    let mut children = Vec::new();
    let mut file_paths = Vec::new();

    // Add the directory itself as a file path
    file_paths.push(dir_path.to_string_lossy().to_string());

    if dir_path.exists() && dir_path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(dir_path)
            .context("Failed to read directory")?
            .filter_map(|e| e.ok())
            .collect();

        // Sort for consistent output
        entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        for entry in entries {
            let entry_path = entry.path();
            let entry_name = entry.file_name().to_string_lossy().to_string();

            if entry_path.is_dir() {
                // Determine class name from directory name
                let child_class = match entry_name.as_str() {
                    "Workspace" => "Workspace",
                    "ReplicatedStorage" => "ReplicatedStorage",
                    "ReplicatedFirst" => "ReplicatedFirst",
                    "ServerScriptService" => "ServerScriptService",
                    "ServerStorage" => "ServerStorage",
                    "StarterGui" => "StarterGui",
                    "StarterPack" => "StarterPack",
                    "StarterPlayer" => "StarterPlayer",
                    "StarterPlayerScripts" => "StarterPlayerScripts",
                    "StarterCharacterScripts" => "StarterCharacterScripts",
                    "Lighting" => "Lighting",
                    "SoundService" => "SoundService",
                    "Chat" => "Chat",
                    "Teams" => "Teams",
                    _ => "Folder",
                };

                // Check if directory has an init file
                let has_init = entry_path.join("init.luau").exists()
                    || entry_path.join("init.server.luau").exists()
                    || entry_path.join("init.client.luau").exists();

                let actual_class = if has_init {
                    if entry_path.join("init.server.luau").exists() {
                        "Script"
                    } else if entry_path.join("init.client.luau").exists() {
                        "LocalScript"
                    } else {
                        "ModuleScript"
                    }
                } else {
                    child_class
                };

                if include_non_scripts || has_init || actual_class != "Folder" {
                    let child_node =
                        build_sourcemap_node(&entry_name, actual_class, &entry_path, include_non_scripts)?;
                    children.push(child_node);
                }
            } else if let Some(ext) = entry_path.extension() {
                if ext == "luau" || ext == "lua" {
                    // Script file
                    let (script_name, script_class) = parse_script_name(&entry_name);

                    // Skip init files (handled by directory)
                    if script_name == "init" {
                        continue;
                    }

                    children.push(serde_json::json!({
                        "name": script_name,
                        "className": script_class,
                        "filePaths": [entry_path.to_string_lossy()]
                    }));
                } else if ext == "rbxjson" && include_non_scripts {
                    // Instance JSON file
                    let instance_name = entry_path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // Try to read class name from JSON
                    let class_name = if let Ok(content) = std::fs::read_to_string(&entry_path) {
                        serde_json::from_str::<serde_json::Value>(&content)
                            .ok()
                            .and_then(|v| v.get("className").and_then(|c| c.as_str()).map(String::from))
                            .unwrap_or_else(|| "Instance".to_string())
                    } else {
                        "Instance".to_string()
                    };

                    children.push(serde_json::json!({
                        "name": instance_name,
                        "className": class_name,
                        "filePaths": [entry_path.to_string_lossy()]
                    }));
                }
            }
        }
    }

    Ok(serde_json::json!({
        "name": name,
        "className": class_name,
        "filePaths": file_paths,
        "children": children
    }))
}

/// Parse script name and class from filename
fn parse_script_name(filename: &str) -> (String, &'static str) {
    let name = filename
        .trim_end_matches(".luau")
        .trim_end_matches(".lua");

    if name.ends_with(".server") {
        (name.trim_end_matches(".server").to_string(), "Script")
    } else if name.ends_with(".client") {
        (name.trim_end_matches(".client").to_string(), "LocalScript")
    } else {
        (name.to_string(), "ModuleScript")
    }
}

/// Build a .rbxl or .rbxm file from project files
fn cmd_build(path: Option<PathBuf>, output: Option<PathBuf>, format: String) -> Result<()> {
    let project_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
    let src_dir = project_dir.join("src");

    if !src_dir.exists() {
        anyhow::bail!("Source directory not found: {}", src_dir.display());
    }

    let format = format.to_lowercase();
    let extension = match format.as_str() {
        "rbxl" | "place" => "rbxl",
        "rbxm" | "model" => "rbxm",
        _ => anyhow::bail!("Unknown format: {}. Use 'rbxl' or 'rbxm'", format),
    };

    let output_path = output.unwrap_or_else(|| {
        std::fs::create_dir_all(project_dir.join("build")).ok();
        project_dir.join(format!("build/game.{}", extension))
    });

    println!("Building {} from {:?}...", extension, src_dir);

    // Build the DOM
    let dom = build_dom_from_src(&src_dir, extension == "rbxl")?;

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    // Write to file
    let output_file = BufWriter::new(
        File::create(&output_path).context("Failed to create output file")?,
    );

    let root_refs = vec![dom.root_ref()];
    rbx_binary::to_writer(output_file, &dom, &root_refs)
        .context("Failed to write output file")?;

    println!("Built successfully: {}", output_path.display());

    // Show file size
    if let Ok(metadata) = std::fs::metadata(&output_path) {
        println!("Size: {:.1} KB", metadata.len() as f64 / 1024.0);
    }

    Ok(())
}

/// Build a DOM from the src directory
fn build_dom_from_src(src_dir: &std::path::Path, is_place: bool) -> Result<WeakDom> {
    let root_class = if is_place { "DataModel" } else { "Folder" };
    let root_name = if is_place { "game" } else { "Model" };

    let mut dom = WeakDom::new(InstanceBuilder::new(root_class).with_name(root_name));
    let root_ref = dom.root_ref();

    // Process each service directory
    let mut entries: Vec<_> = std::fs::read_dir(src_dir)
        .context("Failed to read src directory")?
        .filter_map(|e| e.ok())
        .collect();

    entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    for entry in entries {
        let entry_path = entry.path();
        let entry_name = entry.file_name().to_string_lossy().to_string();

        if entry_path.is_dir() {
            // Directory becomes a service or folder
            let class_name = service_class_name(&entry_name);
            let service_ref = dom.insert(
                root_ref,
                InstanceBuilder::new(class_name).with_name(&entry_name),
            );

            // Recursively add children
            build_dom_children(&mut dom, service_ref, &entry_path)?;
        } else if entry_path.extension().map(|e| e == "rbxjson").unwrap_or(false) {
            // .rbxjson file becomes an instance
            let instance_name = entry_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            if let Ok(content) = std::fs::read_to_string(&entry_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    let class_name = json
                        .get("className")
                        .and_then(|c| c.as_str())
                        .unwrap_or("Folder");

                    let mut builder = InstanceBuilder::new(class_name).with_name(&instance_name);

                    // Add properties from JSON
                    if let Some(props) = json.get("properties").and_then(|p| p.as_object()) {
                        for (prop_name, prop_value) in props {
                            if let Some(value) = json_to_variant(prop_value) {
                                builder = builder.with_property(prop_name, value);
                            }
                        }
                    }

                    dom.insert(root_ref, builder);
                }
            }
        } else if entry_path.extension().map(|e| e == "luau" || e == "lua").unwrap_or(false) {
            // Script file
            let (script_name, class_name) = parse_script_name(&entry_name);
            if let Ok(source) = std::fs::read_to_string(&entry_path) {
                dom.insert(
                    root_ref,
                    InstanceBuilder::new(class_name)
                        .with_name(&script_name)
                        .with_property("Source", Variant::String(source)),
                );
            }
        }
    }

    Ok(dom)
}

/// Recursively build DOM children from a directory
fn build_dom_children(
    dom: &mut WeakDom,
    parent_ref: rbx_dom_weak::types::Ref,
    dir_path: &std::path::Path,
) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir_path)
        .context("Failed to read directory")?
        .filter_map(|e| e.ok())
        .collect();

    entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    // Check for init file first
    let init_files = ["init.luau", "init.server.luau", "init.client.luau"];
    for init_name in init_files {
        let init_path = dir_path.join(init_name);
        if init_path.exists() {
            if let Ok(source) = std::fs::read_to_string(&init_path) {
                // Set Source property on parent
                if let Some(instance) = dom.get_by_ref_mut(parent_ref) {
                    instance
                        .properties
                        .insert("Source".to_string(), Variant::String(source));
                }
            }
            break;
        }
    }

    for entry in entries {
        let entry_path = entry.path();
        let entry_name = entry.file_name().to_string_lossy().to_string();

        // Skip init files
        if init_files.iter().any(|&n| entry_name == n) {
            continue;
        }

        if entry_path.is_dir() {
            // Check if directory has init file (makes it a script)
            let has_init = init_files.iter().any(|&n| entry_path.join(n).exists());

            let class_name = if has_init {
                if entry_path.join("init.server.luau").exists() {
                    "Script"
                } else if entry_path.join("init.client.luau").exists() {
                    "LocalScript"
                } else {
                    "ModuleScript"
                }
            } else {
                "Folder"
            };

            let child_ref = dom.insert(
                parent_ref,
                InstanceBuilder::new(class_name).with_name(&entry_name),
            );

            build_dom_children(dom, child_ref, &entry_path)?;
        } else if entry_path.extension().map(|e| e == "rbxjson").unwrap_or(false) {
            // .rbxjson file
            let instance_name = entry_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            if let Ok(content) = std::fs::read_to_string(&entry_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    let class_name = json
                        .get("className")
                        .and_then(|c| c.as_str())
                        .unwrap_or("Folder");

                    let mut builder = InstanceBuilder::new(class_name).with_name(&instance_name);

                    if let Some(props) = json.get("properties").and_then(|p| p.as_object()) {
                        for (prop_name, prop_value) in props {
                            if let Some(value) = json_to_variant(prop_value) {
                                builder = builder.with_property(prop_name, value);
                            }
                        }
                    }

                    dom.insert(parent_ref, builder);
                }
            }
        } else if entry_path.extension().map(|e| e == "luau" || e == "lua").unwrap_or(false) {
            // Script file
            let (script_name, class_name) = parse_script_name(&entry_name);
            if let Ok(source) = std::fs::read_to_string(&entry_path) {
                dom.insert(
                    parent_ref,
                    InstanceBuilder::new(class_name)
                        .with_name(&script_name)
                        .with_property("Source", Variant::String(source)),
                );
            }
        }
    }

    Ok(())
}

/// Get the appropriate class name for a service directory
fn service_class_name(name: &str) -> &'static str {
    match name {
        "Workspace" => "Workspace",
        "ReplicatedStorage" => "ReplicatedStorage",
        "ReplicatedFirst" => "ReplicatedFirst",
        "ServerScriptService" => "ServerScriptService",
        "ServerStorage" => "ServerStorage",
        "StarterGui" => "StarterGui",
        "StarterPack" => "StarterPack",
        "StarterPlayer" => "StarterPlayer",
        "Lighting" => "Lighting",
        "SoundService" => "SoundService",
        "Chat" => "Chat",
        "Teams" => "Teams",
        "TestService" => "TestService",
        "Players" => "Players",
        _ => "Folder",
    }
}

/// Convert JSON property value to rbx_dom Variant
fn json_to_variant(value: &serde_json::Value) -> Option<Variant> {
    // Check if it has a type field (our format)
    if let Some(obj) = value.as_object() {
        if let Some(type_str) = obj.get("type").and_then(|t| t.as_str()) {
            let val = obj.get("value")?;
            return match type_str {
                "string" => val.as_str().map(|s| Variant::String(s.to_string())),
                "int" | "int32" => val.as_i64().map(|n| Variant::Int32(n as i32)),
                "int64" => val.as_i64().map(Variant::Int64),
                "float" | "float32" => val.as_f64().map(|n| Variant::Float32(n as f32)),
                "float64" => val.as_f64().map(Variant::Float64),
                "bool" => val.as_bool().map(Variant::Bool),
                _ => None,
            };
        }
    }

    // Direct value
    match value {
        serde_json::Value::String(s) => Some(Variant::String(s.clone())),
        serde_json::Value::Bool(b) => Some(Variant::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(Variant::Int32(i as i32))
            } else {
                n.as_f64().map(|f| Variant::Float64(f))
            }
        }
        _ => None,
    }
}
