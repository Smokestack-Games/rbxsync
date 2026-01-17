//! RbxSync CLI
//!
//! Command-line interface for Roblox game extraction and synchronization.

use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use rbx_dom_weak::types::Variant;
use rbx_dom_weak::{InstanceBuilder, WeakDom};
use rbxsync_core::{
    build_plugin, find_rojo_project, get_studio_plugins_folder, install_plugin, parse_rojo_project,
    rojo_to_tree_mapping, PluginBuildConfig, ProjectConfig,
};
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

        /// Skip generating sourcemap.json
        #[arg(long)]
        no_sourcemap: bool,
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

        /// Include terrain data (opt-in, can be slow)
        #[arg(long)]
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

        /// Run server in background (detached)
        #[arg(short, long)]
        background: bool,
    },

    /// Stop the running sync server
    Stop {
        /// Port to stop (default: 44755, or "all" to stop all rbxsync servers)
        #[arg(short, long, default_value = "44755")]
        port: String,
    },

    /// Show sync status
    Status,

    /// Show diff between local files and Studio
    Diff,

    /// Sync local changes to connected Studio instance
    Sync {
        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Keep orphaned instances in Studio (by default, they are deleted)
        #[arg(long)]
        no_delete: bool,
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

        /// Output format: rbxl, rbxm, rbxlx (XML place), or rbxmx (XML model)
        #[arg(short, long, default_value = "rbxl")]
        format: String,

        /// Watch for file changes and rebuild automatically
        #[arg(short, long)]
        watch: bool,

        /// Output to Studio plugins folder with this filename (e.g., MyPlugin.rbxm)
        #[arg(long)]
        plugin: Option<String>,
    },

    /// Format project JSON files with consistent style
    FmtProject {
        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Check formatting without writing (exit 1 if unformatted)
        #[arg(long)]
        check: bool,
    },

    /// Open RbxSync documentation in browser
    Doc,

    /// Update RbxSync (pull latest, rebuild CLI and plugin)
    Update {
        /// Skip git pull (just rebuild)
        #[arg(long)]
        no_pull: bool,

        /// Also rebuild VS Code extension
        #[arg(long)]
        vscode: bool,
    },

    /// Uninstall RbxSync (remove CLI, plugin, and optionally VS Code extension)
    Uninstall {
        /// Also remove VS Code extension
        #[arg(long)]
        vscode: bool,

        /// Keep the cloned repo at ~/.rbxsync/repo
        #[arg(long)]
        keep_repo: bool,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// Show current version and check for updates
    Version,

    /// Migrate from Rojo project to RbxSync
    Migrate {
        /// Source format (currently only "rojo" is supported)
        #[arg(long, default_value = "rojo")]
        from: String,

        /// Path to project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Overwrite existing rbxsync.json
        #[arg(long)]
        force: bool,
    },

    /// Start the Flux agent (control Studio via iMessage)
    Flux {
        /// Run in local mode (terminal testing, no iMessage)
        #[arg(long)]
        local: bool,

        /// Set the Anthropic API key
        #[arg(long)]
        set_api_key: Option<String>,
    },

    /// Manage AI development harness (multi-session tracking)
    Harness {
        #[command(subcommand)]
        action: HarnessAction,
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

#[derive(Subcommand)]
enum HarnessAction {
    /// Initialize harness for a project
    Init {
        /// Game name
        #[arg(short, long)]
        name: String,

        /// Game genre (e.g., RPG, Simulator, Obby)
        #[arg(short, long)]
        genre: Option<String>,

        /// Game description
        #[arg(short, long)]
        description: Option<String>,

        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// Show harness status for the project
    Status {
        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// List features
    Features {
        /// Filter by status (planned, in_progress, completed, blocked, cancelled)
        #[arg(short, long)]
        status: Option<String>,

        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// Create or update a feature
    Feature {
        /// Feature name (creates new feature if no --id is provided)
        name: String,

        /// Feature ID (for updating existing feature)
        #[arg(long)]
        id: Option<String>,

        /// Feature status (planned, in_progress, completed, blocked, cancelled)
        #[arg(short, long)]
        status: Option<String>,

        /// Feature description
        #[arg(short, long)]
        description: Option<String>,

        /// Feature priority (low, medium, high, critical)
        #[arg(long)]
        priority: Option<String>,

        /// Add a note to the feature
        #[arg(long)]
        note: Option<String>,

        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// Manage development sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// Start a new development session
    Start {
        /// Initial goals for the session
        #[arg(short, long)]
        goals: Option<String>,

        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// End the current session
    End {
        /// Session ID to end
        #[arg(short, long)]
        id: String,

        /// Summary of what was accomplished
        #[arg(short, long)]
        summary: Option<String>,

        /// Handoff notes for future sessions
        #[arg(long)]
        handoff: Option<Vec<String>>,

        /// Project directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
}

/// Check for duplicate rbxsync installations that might cause version confusion
fn check_duplicate_installations() {
    // Skip if we're being called recursively (to check version)
    if std::env::var("RBXSYNC_VERSION_CHECK").is_ok() {
        return;
    }

    let current_exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => return,
    };

    let current_version = env!("CARGO_PKG_VERSION");

    // Common installation paths to check
    let paths_to_check = [
        "/usr/local/bin/rbxsync",
        "/usr/bin/rbxsync",
        &format!("{}/.cargo/bin/rbxsync", std::env::var("HOME").unwrap_or_default()),
    ];

    for path_str in &paths_to_check {
        let path = std::path::Path::new(path_str);

        // Skip if it's the same as current exe or doesn't exist
        if !path.exists() {
            continue;
        }

        if let Ok(canonical_current) = current_exe.canonicalize() {
            if let Ok(canonical_other) = path.canonicalize() {
                if canonical_current == canonical_other {
                    continue;
                }
            }
        }

        // Found a different installation - check its version
        // Set env var to prevent recursive check
        if let Ok(output) = std::process::Command::new(path)
            .arg("--version")
            .env("RBXSYNC_VERSION_CHECK", "1")
            .output()
        {
            let version_output = String::from_utf8_lossy(&output.stdout);
            let other_version = version_output
                .split_whitespace()
                .last()
                .unwrap_or("unknown");

            if other_version != current_version {
                eprintln!("⚠️  Warning: Multiple rbxsync installations detected with different versions!");
                eprintln!("   Running:  {} (v{})", current_exe.display(), current_version);
                eprintln!("   Found:    {} (v{})", path_str, other_version);
                eprintln!();
                eprintln!("   This can cause confusion. To fix, remove the older version:");
                eprintln!("   sudo rm {}", path_str);
                eprintln!();
            }
        }
    }
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

    // Check for duplicate installations that might cause confusion
    check_duplicate_installations();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name, path, no_sourcemap } => {
            cmd_init(name, path, no_sourcemap).await?;
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
        Commands::Serve { port, background } => {
            cmd_serve(port, background).await?;
        }
        Commands::Stop { port } => {
            cmd_stop(&port).await?;
        }
        Commands::Status => {
            cmd_status().await?;
        }
        Commands::Diff => {
            cmd_diff().await?;
        }
        Commands::Sync { path, no_delete } => {
            cmd_sync(path, !no_delete).await?;
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
            watch,
            plugin,
        } => {
            cmd_build(path, output, format, watch, plugin).await?;
        }
        Commands::FmtProject { path, check } => {
            cmd_fmt_project(path, check)?;
        }
        Commands::Doc => {
            cmd_doc()?;
        }
        Commands::Update { no_pull, vscode } => {
            cmd_update(no_pull, vscode)?;
        }
        Commands::Version => {
            cmd_version().await?;
        }
        Commands::Flux { local, set_api_key } => {
            // Flux agent is not yet implemented in the CLI
            println!("Flux agent coming soon. Use the flux-agent npm package directly for now.");
            if local {
                println!("  --local flag noted");
            }
            if let Some(key) = set_api_key {
                println!("  API key would be set to: {}...", &key[..8.min(key.len())]);
            }
        }
        Commands::Uninstall { vscode, keep_repo, yes } => {
            cmd_uninstall(vscode, keep_repo, yes)?;
        }
        Commands::Migrate { from, path, force } => {
            cmd_migrate(from, path, force)?;
        }
        Commands::Harness { action } => {
            cmd_harness(action).await?;
        }
    }

    Ok(())
}

/// Initialize a new project
async fn cmd_init(name: Option<String>, path: Option<PathBuf>, no_sourcemap: bool) -> Result<()> {
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

    // Create or update .gitignore (append entries instead of overwriting)
    let gitignore_path = project_dir.join(".gitignore");
    let rbxsync_entries = [".rbxsync/", "*.rbxl", "*.rbxlx", ".DS_Store", "Thumbs.db"];

    let existing_content = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    let existing_lines: HashSet<&str> = existing_content.lines().map(|l| l.trim()).collect();
    let mut additions: Vec<&str> = Vec::new();

    for entry in &rbxsync_entries {
        if !existing_lines.contains(entry) {
            additions.push(entry);
        }
    }

    if !additions.is_empty() {
        let mut new_content = existing_content.clone();
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str("\n# RbxSync\n");
        for entry in additions {
            new_content.push_str(entry);
            new_content.push('\n');
        }
        std::fs::write(&gitignore_path, new_content).context("Failed to write .gitignore")?;
    } else if !gitignore_path.exists() {
        // Create new .gitignore if it doesn't exist
        let gitignore_content = "# RbxSync\n.rbxsync/\n*.rbxl\n*.rbxlx\n\n# OS files\n.DS_Store\nThumbs.db\n";
        std::fs::write(&gitignore_path, gitignore_content).context("Failed to write .gitignore")?;
    }

    // Generate sourcemap for Luau LSP (unless --no-sourcemap)
    if !no_sourcemap {
        let sourcemap_path = project_dir.join("sourcemap.json");
        let root = build_sourcemap_node("game", "DataModel", &src_dir, false)?;
        let json = serde_json::to_string_pretty(&root)?;
        std::fs::write(&sourcemap_path, json).context("Failed to write sourcemap.json")?;
    }

    println!("Initialized RbxSync project '{}' at {:?}", project_name, project_dir);
    println!("\nProject structure:");
    println!("  rbxsync.json      - Project configuration");
    println!("  src/              - Instance tree");
    println!("  assets/           - Binary assets (meshes, images, sounds)");
    println!("  terrain/          - Terrain voxel data");
    if !no_sourcemap {
        println!("  sourcemap.json    - For Luau LSP");
    }
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
            "include_terrain": terrain,
            "include_assets": assets,
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

/// Detect project structure for zero-config mode
fn detect_project_structure() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;

    // Check for common project structures
    if cwd.join("src").is_dir() {
        return Some("src".to_string());
    }

    // Check for Rojo-style project
    if cwd.join("default.project.json").exists() {
        return Some("rojo".to_string());
    }

    // Check for any .luau or .lua files indicating a Roblox project
    if let Ok(entries) = std::fs::read_dir(&cwd) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "luau" || ext == "lua" {
                    return Some("flat".to_string());
                }
            }
        }
    }

    None
}

/// Start the sync server
async fn cmd_serve(port: u16, background: bool) -> Result<()> {
    let config_path = std::env::current_dir()?.join("rbxsync.json");
    let zero_config_mode = !config_path.exists();

    if zero_config_mode {
        // Zero-config mode: work without rbxsync.json
        let project_structure = detect_project_structure();

        println!("Running in zero-config mode (no rbxsync.json found)");
        println!();

        match project_structure {
            Some(ref structure) if structure == "src" => {
                println!("Detected: src/ directory (standard structure)");
            }
            Some(ref structure) if structure == "rojo" => {
                println!("Detected: Rojo project (default.project.json)");
                println!("Tip: Run `rbxsync migrate` to convert to RbxSync format");
            }
            Some(_) => {
                println!("Detected: Luau files in current directory");
            }
            None => {
                println!("No existing project detected - will create src/ on first extract");
            }
        }

        println!();
        println!("Using defaults:");
        println!("  Source folder: ./src");
        println!("  Assets folder: ./assets");
        println!();
        println!("For more control, create rbxsync.json with: rbxsync init");
        println!();
    } else {
        // Validate JSON is parseable if config exists
        let config_content = std::fs::read_to_string(&config_path)
            .context("Failed to read rbxsync.json")?;

        if let Err(e) = serde_json::from_str::<serde_json::Value>(&config_content) {
            eprintln!("Error: Invalid JSON in rbxsync.json");
            eprintln!();
            eprintln!("Parse error: {}", e);
            eprintln!();
            eprintln!("Please fix the JSON syntax and try again.");
            std::process::exit(1);
        }
    }

    // Check if port is available before attempting to start
    if !is_port_available(port) {
        eprintln!("Error: Port {} is already in use.", port);
        eprintln!();
        eprintln!("This could mean:");
        eprintln!("  - Another rbxsync server is already running");
        eprintln!("  - Another application is using this port");
        eprintln!();
        eprintln!("Try: rbxsync stop --port {}", port);
        eprintln!("Or use a different port: rbxsync serve --port <PORT>");
        std::process::exit(1);
    }

    if background {
        // Spawn server as a detached background process
        let exe = std::env::current_exe()?;
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(["serve", "--port", &port.to_string()]);

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0); // Create new process group
        }

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x00000008;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
        }

        let child = cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Failed to spawn background server")?;

        println!("RbxSync server started in background (PID: {})", child.id());
        println!("  Port: {}", port);
        println!("  Stop with: rbxsync stop");
        return Ok(());
    }

    // Foreground mode
    println!("RbxSync server running on port {}", port);
    println!("Stop with: Ctrl+C or `rbxsync stop` from another terminal");
    println!("Run in background with: rbxsync serve --background");
    run_server(ServerConfig {
        port,
        ..Default::default()
    })
    .await
}

/// Stop the running sync server
async fn cmd_stop(port: &str) -> Result<()> {
    // Handle "all" to stop all rbxsync servers
    if port == "all" {
        return stop_all_servers().await;
    }

    let port_num: u16 = port.parse().context("Invalid port number")?;
    stop_server_on_port(port_num).await
}

/// Stop all rbxsync servers
#[cfg(unix)]
async fn stop_all_servers() -> Result<()> {
    let output = std::process::Command::new("pgrep")
        .args(["-f", "rbxsync.*serve"])
        .output();

    if let Ok(output) = output {
        let pids = String::from_utf8_lossy(&output.stdout);
        let pids: Vec<&str> = pids.lines().filter(|p| !p.is_empty()).collect();

        if pids.is_empty() {
            println!("No rbxsync servers running.");
            return Ok(());
        }

        println!("Stopping {} rbxsync server(s)...", pids.len());
        for pid in pids {
            if let Ok(pid) = pid.trim().parse::<i32>() {
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        println!("All servers stopped.");
    }
    Ok(())
}

#[cfg(not(unix))]
async fn stop_all_servers() -> Result<()> {
    println!("Stopping all servers is only supported on Unix systems.");
    println!("Please specify a port: rbxsync stop --port PORT");
    Ok(())
}

/// Wait for a port to be released, polling up to `timeout_ms` milliseconds
async fn wait_for_port_release(port: u16, timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(timeout_ms);

    while start.elapsed() < timeout {
        // Try to bind to the port - if successful, the port is free
        match std::net::TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(_) => return true,
            Err(_) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
    false
}

/// Check if a port is available
fn is_port_available(port: u16) -> bool {
    std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok()
}

/// Stop server on a specific port
async fn stop_server_on_port(port: u16) -> Result<()> {
    // First, try to find any rbxsync server processes by port
    #[cfg(unix)]
    {
        let port_arg = format!(":{}", port);
        let output = std::process::Command::new("lsof")
            .args(["-ti", &port_arg])
            .output();

        if let Ok(output) = output {
            let pids = String::from_utf8_lossy(&output.stdout);
            let pids: Vec<&str> = pids.lines().filter(|p| !p.is_empty()).collect();

            if pids.is_empty() {
                println!("No server running on port {}.", port);
                return Ok(());
            }

            // Try graceful shutdown first via HTTP
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()?;

            let url = format!("http://localhost:{}/shutdown", port);
            let _ = client.post(&url).send().await;  // Ignore result, check if port is released

            // Wait briefly for graceful shutdown
            if wait_for_port_release(port, 2000).await {
                println!("Server stopped.");
                return Ok(());
            }

            // Graceful shutdown didn't work, try SIGTERM first (allows cleanup)
            println!("Sending SIGTERM to server...");
            for pid in &pids {
                if let Ok(pid) = pid.trim().parse::<i32>() {
                    unsafe {
                        libc::kill(pid, libc::SIGTERM);
                    }
                }
            }

            // Wait for SIGTERM to take effect
            if wait_for_port_release(port, 2000).await {
                println!("Server stopped.");
                return Ok(());
            }

            // SIGTERM didn't work, force kill with SIGKILL
            println!("Force killing server (SIGKILL)...");
            for pid in &pids {
                if let Ok(pid) = pid.trim().parse::<i32>() {
                    unsafe {
                        libc::kill(pid, libc::SIGKILL);
                    }
                }
            }

            // Final check
            if wait_for_port_release(port, 2000).await {
                println!("Server stopped.");
            } else {
                // Last resort: print the PIDs so user can manually kill
                println!("Warning: Could not stop server. Try manually:");
                for pid in &pids {
                    println!("  kill -9 {}", pid.trim());
                }
            }
            return Ok(());
        }
    }

    // Fallback for non-unix or if lsof failed
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;

    let url = format!("http://localhost:{}/shutdown", port);
    match client.post(&url).send().await {
        Ok(_) => {
            println!("Server stopped.");
            Ok(())
        }
        Err(_) => {
            println!("No server running on port {}.", port);
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

/// Show diff between local files and Studio
async fn cmd_diff() -> Result<()> {
    let project_dir = std::env::current_dir().unwrap();
    let project_dir_str = project_dir.to_string_lossy().to_string();

    let client = reqwest::Client::new();

    // Check server is running
    if client.get("http://localhost:44755/health").send().await.is_err() {
        println!("RbxSync server is not running. Start it with: rbxsync serve");
        return Ok(());
    }

    println!("Comparing files with Studio...");

    // Call diff endpoint
    let response = client
        .post("http://localhost:44755/diff")
        .json(&serde_json::json!({
            "project_dir": project_dir_str
        }))
        .send()
        .await
        .context("Failed to get diff")?;

    let diff: serde_json::Value = response.json().await?;

    if diff.get("success").and_then(|v| v.as_bool()) != Some(true) {
        let error = diff.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error");
        println!("Error: {}", error);
        return Ok(());
    }

    let added = diff.get("added").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let removed = diff.get("removed").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let common = diff.get("common").and_then(|v| v.as_u64()).unwrap_or(0);
    let file_count = diff.get("file_count").and_then(|v| v.as_u64()).unwrap_or(0);
    let studio_count = diff.get("studio_count").and_then(|v| v.as_u64()).unwrap_or(0);

    // Print added (in files, not in Studio)
    if !added.is_empty() {
        println!("\n\x1b[32mFiles → Studio (would be created): {}\x1b[0m", added.len());
        for entry in added.iter().take(20) {
            let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let class = entry.get("className").and_then(|v| v.as_str()).unwrap_or("");
            println!("  + {} ({})", path, class);
        }
        if added.len() > 20 {
            println!("  ... and {} more", added.len() - 20);
        }
    }

    // Print removed (in Studio, not in files)
    if !removed.is_empty() {
        println!("\n\x1b[31mStudio only (would be deleted with --delete): {}\x1b[0m", removed.len());
        for entry in removed.iter().take(20) {
            let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let class = entry.get("className").and_then(|v| v.as_str()).unwrap_or("");
            println!("  - {} ({})", path, class);
        }
        if removed.len() > 20 {
            println!("  ... and {} more", removed.len() - 20);
        }
    }

    // Summary
    println!("\n\x1b[1mSummary:\x1b[0m");
    println!("  Files: {} instances", file_count);
    println!("  Studio: {} instances", studio_count);
    println!("  Common: {} (in sync)", common);
    println!("  Added: {} (files → studio)", added.len());
    println!("  Removed: {} (studio only)", removed.len());

    if added.is_empty() && removed.is_empty() {
        println!("\n\x1b[32m✓ Files and Studio are in sync!\x1b[0m");
    }

    Ok(())
}

/// Sync local changes to Studio
async fn cmd_sync(path: Option<PathBuf>, delete: bool) -> Result<()> {
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

    // Build sync operations for updates
    let mut operations: Vec<serde_json::Value> = instances
        .into_iter()
        .map(|inst| {
            serde_json::json!({
                "type": "update",
                "path": inst.get("path"),
                "data": inst
            })
        })
        .collect();

    // If --delete flag is set, get diff and add delete operations
    if delete {
        println!("Checking for orphaned instances in Studio...");
        let diff_response = client
            .post("http://localhost:44755/diff")
            .json(&serde_json::json!({
                "project_dir": project_dir_str
            }))
            .send()
            .await
            .context("Failed to get diff")?;

        let diff: serde_json::Value = diff_response.json().await?;
        let removed = diff.get("removed").and_then(|v| v.as_array()).cloned().unwrap_or_default();

        if !removed.is_empty() {
            println!("Found {} orphaned instances to delete", removed.len());
            for entry in removed {
                let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let class_name = entry.get("class_name").and_then(|v| v.as_str()).unwrap_or("Instance");
                println!("  \x1b[31m- {}\x1b[0m ({})", path, class_name);
                operations.push(serde_json::json!({
                    "type": "delete",
                    "path": path
                }));
            }
        }
    }

    if operations.is_empty() {
        println!("No changes to sync.");
        return Ok(());
    }

    let update_count = operations.iter().filter(|op| op.get("type").and_then(|v| v.as_str()) == Some("update")).count();
    let delete_count = operations.iter().filter(|op| op.get("type").and_then(|v| v.as_str()) == Some("delete")).count();

    if delete_count > 0 {
        println!("Syncing {} updates and {} deletes to Studio...", update_count, delete_count);
    } else {
        println!("Syncing {} instances to Studio...", update_count);
    }

    // Send batch sync
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
        // Use our own counts since server response may not include all operations
        if delete_count > 0 {
            println!("\x1b[32m✓ Synced {} instances, deleted {} orphans.\x1b[0m", update_count, delete_count);
        } else {
            println!("\x1b[32m✓ Synced {} instances to Studio.\x1b[0m", update_count);
        }
    } else {
        let errors = result.get("errors").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        println!("Sync completed with errors:");
        for err in errors {
            println!("  - {}", err);
        }
    }

    // Check for terrain data and sync if present
    let terrain_file = project_dir.join("src").join("Workspace").join("Terrain").join("terrain.rbxjson");
    if terrain_file.exists() {
        println!("Syncing terrain...");

        // Read terrain data
        let terrain_json = std::fs::read_to_string(&terrain_file)
            .context("Failed to read terrain file")?;
        let terrain_data: serde_json::Value = serde_json::from_str(&terrain_json)
            .context("Failed to parse terrain file")?;

        // Send terrain sync command
        let terrain_response = client
            .post("http://localhost:44755/sync/command")
            .json(&serde_json::json!({
                "command": "terrain:sync",
                "payload": {
                    "terrain": terrain_data,
                    "clear": true
                }
            }))
            .send()
            .await
            .context("Failed to sync terrain")?;

        let terrain_result: serde_json::Value = terrain_response.json().await?;

        if terrain_result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
            let chunks = terrain_result.get("data")
                .and_then(|d| d.get("chunksApplied"))
                .and_then(|c| c.as_u64())
                .unwrap_or(0);
            println!("\x1b[32m✓ Synced {} terrain chunks.\x1b[0m", chunks);
        } else {
            let error = terrain_result.get("error")
                .or_else(|| terrain_result.get("data").and_then(|d| d.get("error")))
                .and_then(|e| e.as_str())
                .unwrap_or("Unknown error");
            println!("\x1b[33m⚠ Terrain sync failed: {}\x1b[0m", error);
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
async fn cmd_build(
    path: Option<PathBuf>,
    output: Option<PathBuf>,
    format: String,
    watch: bool,
    plugin: Option<String>,
) -> Result<()> {
    let project_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
    let src_dir = project_dir.join("src");

    if !src_dir.exists() {
        bail!("Source directory not found: {}", src_dir.display());
    }

    // Parse format and determine if XML
    let format = format.to_lowercase();
    let (extension, is_xml) = match format.as_str() {
        "rbxl" | "place" => ("rbxl", false),
        "rbxm" | "model" => ("rbxm", false),
        "rbxlx" | "place-xml" => ("rbxlx", true),
        "rbxmx" | "model-xml" => ("rbxmx", true),
        _ => bail!("Unknown format: {}. Use rbxl, rbxm, rbxlx, or rbxmx", format),
    };

    // Determine output path
    let output_path = if let Some(plugin_name) = &plugin {
        // Output to Studio plugins folder
        let plugins_folder = get_studio_plugins_folder()
            .context("Could not determine Studio plugins folder")?;
        std::fs::create_dir_all(&plugins_folder).ok();
        plugins_folder.join(plugin_name)
    } else if let Some(out) = output {
        out
    } else {
        std::fs::create_dir_all(project_dir.join("build")).ok();
        project_dir.join(format!("build/game.{}", extension))
    };

    // Initial build
    do_build(&src_dir, &output_path, extension, is_xml)?;

    // If not watch mode, we're done
    if !watch {
        return Ok(());
    }

    // Watch mode
    println!("\nWatching for changes... (Ctrl+C to stop)");

    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )
    .context("Failed to create file watcher")?;

    watcher
        .watch(&src_dir, RecursiveMode::Recursive)
        .context("Failed to watch source directory")?;

    // Debounce tracking
    let mut last_build = std::time::Instant::now();
    let debounce = Duration::from_millis(500);

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(_event) => {
                // Debounce: only rebuild if enough time has passed
                if last_build.elapsed() >= debounce {
                    println!("\nChange detected, rebuilding...");
                    match do_build(&src_dir, &output_path, extension, is_xml) {
                        Ok(()) => last_build = std::time::Instant::now(),
                        Err(e) => println!("Build error: {}", e),
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Continue watching
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                println!("Watcher disconnected");
                break;
            }
        }
    }

    Ok(())
}

/// Perform the actual build operation
fn do_build(src_dir: &PathBuf, output_path: &PathBuf, extension: &str, is_xml: bool) -> Result<()> {
    let is_place = extension == "rbxl" || extension == "rbxlx";

    println!("Building {} from {:?}...", extension, src_dir);

    // Build the DOM
    let dom = build_dom_from_src(src_dir, is_place)?;

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    // Write to file
    let output_file = BufWriter::new(
        File::create(output_path).context("Failed to create output file")?,
    );

    // Export the children (services/instances) directly, not the root wrapper
    // For places: services should have null parent referent (top-level in file)
    // For models: instances should be top-level items
    // The rbx_binary crate handles service detection based on class names
    let refs_to_export: Vec<_> = dom.root().children().to_vec();

    if is_xml {
        rbx_xml::to_writer_default(output_file, &dom, &refs_to_export)
            .context("Failed to write XML output file")?;
    } else {
        rbx_binary::to_writer(output_file, &dom, &refs_to_export)
            .context("Failed to write binary output file")?;
    }

    println!("Built successfully: {}", output_path.display());

    // Show file size
    if let Ok(metadata) = std::fs::metadata(output_path) {
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

        // Skip init files and _meta.rbxjson
        if init_files.iter().any(|&n| entry_name == n) || entry_name == "_meta.rbxjson" {
            continue;
        }

        if entry_path.is_dir() {
            // Check if directory has _meta.rbxjson (contains class and properties)
            let meta_path = entry_path.join("_meta.rbxjson");
            let meta_data: Option<serde_json::Value> = if meta_path.exists() {
                std::fs::read_to_string(&meta_path)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
            } else {
                None
            };

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
            } else if let Some(ref meta) = meta_data {
                // Use class from _meta.rbxjson if available
                meta.get("className")
                    .and_then(|c| c.as_str())
                    .unwrap_or("Folder")
            } else {
                "Folder"
            };

            let mut builder = InstanceBuilder::new(class_name).with_name(&entry_name);

            // Apply properties from _meta.rbxjson if available
            if let Some(ref meta) = meta_data {
                if let Some(props) = meta.get("properties").and_then(|p| p.as_object()) {
                    for (prop_name, prop_value) in props {
                        if let Some(value) = json_to_variant(prop_value) {
                            builder = builder.with_property(prop_name, value);
                        }
                    }
                }
            }

            let child_ref = dom.insert(parent_ref, builder);

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
    use rbx_dom_weak::types::*;

    // Check if it has a type field (our format)
    if let Some(obj) = value.as_object() {
        if let Some(type_str) = obj.get("type").and_then(|t| t.as_str()) {
            let val = obj.get("value");
            return match type_str {
                // Basic types
                "string" => val?.as_str().map(|s| Variant::String(s.to_string())),
                "int" | "int32" => val?.as_i64().map(|n| Variant::Int32(n as i32)),
                "int64" => val?.as_i64().map(Variant::Int64),
                "float" | "float32" => val?.as_f64().map(|n| Variant::Float32(n as f32)),
                "float64" | "double" => val?.as_f64().map(Variant::Float64),
                "bool" => val?.as_bool().map(Variant::Bool),

                // nil means "use default" - skip the property entirely
                "nil" => None,

                // Vector types
                "Vector2" => {
                    let v = val?.as_object()?;
                    Some(Variant::Vector2(Vector2::new(
                        v.get("x")?.as_f64()? as f32,
                        v.get("y")?.as_f64()? as f32,
                    )))
                }
                "Vector3" => {
                    let v = val?.as_object()?;
                    Some(Variant::Vector3(Vector3::new(
                        v.get("x")?.as_f64()? as f32,
                        v.get("y")?.as_f64()? as f32,
                        v.get("z")?.as_f64()? as f32,
                    )))
                }

                // Color types
                "Color3" => {
                    let v = val?.as_object()?;
                    Some(Variant::Color3(Color3::new(
                        v.get("r")?.as_f64()? as f32,
                        v.get("g")?.as_f64()? as f32,
                        v.get("b")?.as_f64()? as f32,
                    )))
                }
                "Color3uint8" => {
                    let v = val?.as_object()?;
                    Some(Variant::Color3uint8(Color3uint8::new(
                        v.get("r")?.as_u64()? as u8,
                        v.get("g")?.as_u64()? as u8,
                        v.get("b")?.as_u64()? as u8,
                    )))
                }
                "BrickColor" => {
                    val?.as_u64().map(|n| Variant::BrickColor(BrickColor::from_number(n as u16).unwrap_or(BrickColor::MediumStoneGrey)))
                }

                // UDim types
                "UDim" => {
                    let v = val?.as_object()?;
                    Some(Variant::UDim(UDim::new(
                        v.get("scale")?.as_f64()? as f32,
                        v.get("offset")?.as_i64()? as i32,
                    )))
                }
                "UDim2" => {
                    let v = val?.as_object()?;
                    let x = v.get("x")?.as_object()?;
                    let y = v.get("y")?.as_object()?;
                    Some(Variant::UDim2(UDim2::new(
                        UDim::new(
                            x.get("scale")?.as_f64()? as f32,
                            x.get("offset")?.as_i64()? as i32,
                        ),
                        UDim::new(
                            y.get("scale")?.as_f64()? as f32,
                            y.get("offset")?.as_i64()? as i32,
                        ),
                    )))
                }

                // CFrame
                "CFrame" => {
                    let v = val?.as_object()?;
                    let pos = v.get("position")?.as_array()?;
                    let rot = v.get("rotation")?.as_array()?;
                    if pos.len() >= 3 && rot.len() >= 9 {
                        Some(Variant::CFrame(CFrame::new(
                            Vector3::new(
                                pos[0].as_f64()? as f32,
                                pos[1].as_f64()? as f32,
                                pos[2].as_f64()? as f32,
                            ),
                            Matrix3::new(
                                Vector3::new(rot[0].as_f64()? as f32, rot[1].as_f64()? as f32, rot[2].as_f64()? as f32),
                                Vector3::new(rot[3].as_f64()? as f32, rot[4].as_f64()? as f32, rot[5].as_f64()? as f32),
                                Vector3::new(rot[6].as_f64()? as f32, rot[7].as_f64()? as f32, rot[8].as_f64()? as f32),
                            ),
                        )))
                    } else {
                        None
                    }
                }

                // Enum (store as u32)
                "Enum" => {
                    let v = val?.as_object()?;
                    let enum_value = v.get("value")?;
                    // Try to get numeric value, or parse from string
                    if let Some(n) = enum_value.as_u64() {
                        Some(Variant::Enum(rbx_dom_weak::types::Enum::from_u32(n as u32)))
                    } else {
                        // For string enum values, we'd need the reflection database
                        // For now, default to 0
                        Some(Variant::Enum(rbx_dom_weak::types::Enum::from_u32(0)))
                    }
                }

                // Rect
                "Rect" => {
                    let v = val?.as_object()?;
                    let min = v.get("min")?.as_object()?;
                    let max = v.get("max")?.as_object()?;
                    Some(Variant::Rect(Rect::new(
                        Vector2::new(min.get("x")?.as_f64()? as f32, min.get("y")?.as_f64()? as f32),
                        Vector2::new(max.get("x")?.as_f64()? as f32, max.get("y")?.as_f64()? as f32),
                    )))
                }

                // NumberRange
                "NumberRange" => {
                    let v = val?.as_object()?;
                    Some(Variant::NumberRange(NumberRange::new(
                        v.get("min")?.as_f64()? as f32,
                        v.get("max")?.as_f64()? as f32,
                    )))
                }

                // Font
                "Font" => {
                    let v = val?.as_object()?;
                    let family = v.get("family")?.as_str()?.to_string();
                    let weight = v.get("weight").and_then(|w| w.as_u64()).unwrap_or(400) as u16;
                    let style = v.get("style").and_then(|s| s.as_str()).unwrap_or("Normal");
                    Some(Variant::Font(Font {
                        family,
                        weight: FontWeight::from_u16(weight).unwrap_or(FontWeight::Regular),
                        style: if style == "Italic" { FontStyle::Italic } else { FontStyle::Normal },
                        cached_face_id: None,
                    }))
                }

                // Content (asset URLs)
                "Content" => {
                    val?.as_str().map(|s| Variant::Content(Content::from(s.to_string())))
                }

                // Refs - we skip these as they need special handling
                "Ref" => None,

                // Skip unknown/unsupported types
                _ => {
                    tracing::debug!("Unsupported property type: {}", type_str);
                    None
                }
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

/// Format project JSON files with consistent style
fn cmd_fmt_project(path: Option<PathBuf>, check: bool) -> Result<()> {
    let project_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
    let src_dir = project_dir.join("src");

    if !src_dir.exists() {
        bail!("Source directory not found: {}", src_dir.display());
    }

    let mut unformatted = Vec::new();
    let mut formatted_count = 0;

    // Recursively find all .rbxjson files
    fn visit_dir(
        dir: &std::path::Path,
        check: bool,
        unformatted: &mut Vec<PathBuf>,
        formatted_count: &mut usize,
    ) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir).context("Failed to read directory")?;

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                visit_dir(&path, check, unformatted, formatted_count)?;
            } else if path.extension().map_or(false, |ext| ext == "rbxjson") {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;

                // Parse and re-serialize with consistent formatting
                let value: serde_json::Value = serde_json::from_str(&content)
                    .with_context(|| format!("Failed to parse {}", path.display()))?;

                let formatted = serde_json::to_string_pretty(&value)? + "\n";

                if content != formatted {
                    if check {
                        unformatted.push(path);
                    } else {
                        std::fs::write(&path, &formatted)
                            .with_context(|| format!("Failed to write {}", path.display()))?;
                        println!("Formatted: {}", path.display());
                        *formatted_count += 1;
                    }
                }
            }
        }

        Ok(())
    }

    visit_dir(&src_dir, check, &mut unformatted, &mut formatted_count)?;

    // Also format rbxsync.json if it exists
    let config_path = project_dir.join("rbxsync.json");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                let formatted = serde_json::to_string_pretty(&value)? + "\n";
                if content != formatted {
                    if check {
                        unformatted.push(config_path);
                    } else {
                        std::fs::write(&config_path, &formatted)?;
                        println!("Formatted: {}", config_path.display());
                        formatted_count += 1;
                    }
                }
            }
        }
    }

    if check {
        if unformatted.is_empty() {
            println!("All files are properly formatted.");
        } else {
            println!("The following files need formatting:");
            for path in &unformatted {
                println!("  {}", path.display());
            }
            std::process::exit(1);
        }
    } else if formatted_count == 0 {
        println!("All files are already properly formatted.");
    } else {
        println!("\nFormatted {} file(s).", formatted_count);
    }

    Ok(())
}

/// Open documentation in browser
fn cmd_doc() -> Result<()> {
    let doc_url = "https://rbxsync.dev";

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(doc_url)
            .spawn()
            .context("Failed to open browser")?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", doc_url])
            .spawn()
            .context("Failed to open browser")?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(doc_url)
            .spawn()
            .context("Failed to open browser")?;
    }

    println!("Opening documentation: {}", doc_url);
    Ok(())
}

/// Update RbxSync - pull latest changes, rebuild CLI and plugin
fn cmd_update(no_pull: bool, vscode: bool) -> Result<()> {
    // Find repo in this order:
    // 1. Current directory (if it's the repo)
    // 2. ~/.rbxsync/repo (managed clone)
    // 3. Walk up from executable (dev builds)
    // 4. Auto-clone to ~/.rbxsync/repo

    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let managed_repo = home_dir.join(".rbxsync").join("repo");

    let repo_dir = if cwd.join("Cargo.toml").exists() && cwd.join("plugin").exists() {
        // We're in the repo directory
        cwd
    } else if managed_repo.join("Cargo.toml").exists() && managed_repo.join("plugin").exists() {
        // Use managed repo at ~/.rbxsync/repo
        managed_repo
    } else {
        // Try to find repo from executable path (for dev builds)
        let exe_path = std::env::current_exe().context("Failed to get executable path")?;
        let mut found_dir = exe_path.parent().map(|p| p.to_path_buf());

        for _ in 0..5 {
            if let Some(ref dir) = found_dir {
                if dir.join("Cargo.toml").exists() && dir.join("plugin").exists() {
                    break;
                }
                found_dir = dir.parent().map(|p| p.to_path_buf());
            }
        }

        match found_dir {
            Some(dir) if dir.join("Cargo.toml").exists() && dir.join("plugin").exists() => dir,
            _ => {
                // No repo found - clone to ~/.rbxsync/repo
                println!("RbxSync repository not found. Cloning to ~/.rbxsync/repo...");
                println!();

                // Create ~/.rbxsync directory
                std::fs::create_dir_all(&home_dir.join(".rbxsync"))
                    .context("Failed to create ~/.rbxsync directory")?;

                // Clone the repo
                let status = std::process::Command::new("git")
                    .args([
                        "clone",
                        "https://github.com/devmarissa/rbxsync.git",
                        managed_repo.to_str().unwrap(),
                    ])
                    .status()
                    .context("Failed to clone repository. Is git installed?")?;

                if !status.success() {
                    bail!("Failed to clone repository");
                }

                println!("Repository cloned successfully!");
                println!();

                managed_repo
            }
        }
    };

    println!("RbxSync Update");
    println!("==============");
    println!("Repository: {}", repo_dir.display());
    println!();

    // Step 1: Git pull (unless --no-pull)
    if !no_pull {
        println!("1. Pulling latest changes...");
        let status = std::process::Command::new("git")
            .args(["pull", "--ff-only"])
            .current_dir(&repo_dir)
            .status()
            .context("Failed to run git pull")?;

        if !status.success() {
            println!("   Warning: git pull failed. You may have local changes.");
            println!("   Run with --no-pull to skip this step.");
        } else {
            println!("   Done!");
        }
        println!();
    }

    // Step 2: Rebuild CLI
    println!("2. Rebuilding CLI...");
    let status = std::process::Command::new("cargo")
        .args(["build", "--release", "-p", "rbxsync"])
        .current_dir(&repo_dir)
        .status()
        .context("Failed to build CLI")?;

    if !status.success() {
        bail!("Failed to build CLI");
    }
    println!("   Done!");

    // Install CLI to system PATH
    let new_binary = repo_dir.join("target/release/rbxsync");
    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;

    // Only copy if current exe is in a system location (not in target/)
    if !current_exe.to_string_lossy().contains("target") {
        println!("   Installing to {}...", current_exe.display());

        #[cfg(unix)]
        {
            // Try to copy directly, fall back to sudo
            if std::fs::copy(&new_binary, &current_exe).is_err() {
                // Need elevated permissions
                let status = std::process::Command::new("sudo")
                    .args(["cp", new_binary.to_str().unwrap(), current_exe.to_str().unwrap()])
                    .status();

                match status {
                    Ok(s) if s.success() => println!("   Installed!"),
                    _ => {
                        println!("   Could not auto-install. Run manually:");
                        println!("   sudo cp {} {}", new_binary.display(), current_exe.display());
                    }
                }
            } else {
                println!("   Installed!");
            }
        }

        #[cfg(windows)]
        {
            if std::fs::copy(&new_binary, &current_exe).is_err() {
                println!("   Could not auto-install. Run manually as Administrator:");
                println!("   copy {} {}", new_binary.display(), current_exe.display());
            } else {
                println!("   Installed!");
            }
        }
    }
    println!();

    // Step 3: Rebuild and install plugin
    println!("3. Rebuilding and installing plugin...");
    let plugin_config = PluginBuildConfig {
        source_dir: repo_dir.join("plugin/src"),
        output_path: repo_dir.join("build/RbxSync.rbxm"),
        plugin_name: "RbxSync".to_string(),
    };

    build_plugin(&plugin_config).context("Failed to build plugin")?;
    install_plugin(&repo_dir.join("build/RbxSync.rbxm"), "RbxSync")
        .context("Failed to install plugin")?;
    println!("   Done!");
    println!();

    // Step 4: VS Code extension (optional)
    if vscode {
        println!("4. Rebuilding VS Code extension...");
        let vscode_dir = repo_dir.join("rbxsync-vscode");

        if vscode_dir.exists() {
            // npm install
            let status = std::process::Command::new("npm")
                .args(["install"])
                .current_dir(&vscode_dir)
                .status()
                .context("Failed to run npm install")?;

            if !status.success() {
                println!("   Warning: npm install failed");
            }

            // npm run build
            let status = std::process::Command::new("npm")
                .args(["run", "build"])
                .current_dir(&vscode_dir)
                .status()
                .context("Failed to run npm build")?;

            if !status.success() {
                println!("   Warning: npm build failed");
            }

            // npm run package
            let status = std::process::Command::new("npm")
                .args(["run", "package"])
                .current_dir(&vscode_dir)
                .status()
                .context("Failed to run npm package")?;

            if !status.success() {
                println!("   Warning: npm package failed");
            } else {
                println!("   Built: rbxsync-vscode/rbxsync-*.vsix");
                println!("   Install with: code --install-extension rbxsync-vscode/rbxsync-*.vsix");
            }
        } else {
            println!("   Warning: VS Code extension directory not found");
        }
        println!();
    }

    println!("Update complete!");
    println!();
    println!("Next steps:");
    println!("  1. Restart Roblox Studio to load the updated plugin");
    if vscode {
        println!("  2. Install the VS Code extension: code --install-extension rbxsync-vscode/rbxsync-*.vsix");
        println!("  3. Restart VS Code");
    }

    Ok(())
}

/// Compare semver versions (returns true if latest > current)
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<&str> = v.trim_start_matches('v').split('.').collect();
        (
            parts.first().and_then(|s| s.parse().ok()).unwrap_or(0),
            parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0),
            parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0),
        )
    };
    let (l_maj, l_min, l_pat) = parse(latest);
    let (c_maj, c_min, c_pat) = parse(current);
    if l_maj != c_maj { return l_maj > c_maj; }
    if l_min != c_min { return l_min > c_min; }
    l_pat > c_pat
}

/// Show version information
async fn cmd_version() -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");

    println!("RbxSync v{}", version);
    println!();

    // Try to get git info
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("Git commit: {}", commit);
        }
    }

    // Check for updates from GitHub releases
    println!();
    print!("Checking for updates... ");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    match client
        .get("https://api.github.com/repos/devmarissa/rbxsync/releases/latest")
        .header("User-Agent", "rbxsync-cli")
        .send()
        .await
    {
        Ok(response) => {
            if let Ok(release) = response.json::<serde_json::Value>().await {
                if let Some(tag) = release.get("tag_name").and_then(|t| t.as_str()) {
                    let latest = tag.trim_start_matches('v');
                    if is_newer_version(latest, version) {
                        println!("\x1b[33mUpdate available: v{}\x1b[0m", latest);
                        println!("  Run: rbxsync update");
                        println!("  Or download: https://github.com/devmarissa/rbxsync/releases/latest");
                    } else {
                        println!("\x1b[32mUp to date!\x1b[0m");
                    }
                } else {
                    println!("Could not parse version");
                }
            } else {
                println!("Could not parse response");
            }
        }
        Err(_) => {
            println!("Could not check (offline?)");
        }
    }

    println!();
    println!("Documentation: https://rbxsync.dev");

    Ok(())
}

/// Uninstall RbxSync completely
fn cmd_uninstall(vscode: bool, keep_repo: bool, yes: bool) -> Result<()> {
    println!("RbxSync Uninstaller");
    println!("===================");
    println!();

    // Gather what will be removed
    let mut items_to_remove: Vec<(String, PathBuf)> = Vec::new();

    // 1. CLI binary
    let current_exe = std::env::current_exe().ok();
    if let Some(ref exe) = current_exe {
        // Only list if it's in a system location (not in target/)
        if !exe.to_string_lossy().contains("target") {
            items_to_remove.push(("CLI binary".to_string(), exe.clone()));
        }
    }

    // 2. Studio plugin
    if let Some(plugins_folder) = get_studio_plugins_folder() {
        let plugin_path = plugins_folder.join("RbxSync.rbxm");
        if plugin_path.exists() {
            items_to_remove.push(("Studio plugin".to_string(), plugin_path));
        }
    }

    // 3. Managed repo at ~/.rbxsync
    let home_dir = dirs::home_dir();
    let rbxsync_dir = home_dir.as_ref().map(|h| h.join(".rbxsync"));
    if !keep_repo {
        if let Some(ref dir) = rbxsync_dir {
            if dir.exists() {
                items_to_remove.push(("Data directory (~/.rbxsync)".to_string(), dir.clone()));
            }
        }
    }

    // 4. VS Code extension (optional)
    let vscode_extension_id = "rbxsync.rbxsync";

    if items_to_remove.is_empty() && !vscode {
        println!("Nothing to uninstall. RbxSync does not appear to be installed.");
        return Ok(());
    }

    // Show what will be removed
    println!("The following will be removed:");
    println!();
    for (name, path) in &items_to_remove {
        println!("  - {} ({})", name, path.display());
    }
    if vscode {
        println!("  - VS Code extension ({})", vscode_extension_id);
    }
    println!();

    // Confirm unless --yes
    if !yes {
        print!("Are you sure you want to uninstall? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            println!("Uninstall cancelled.");
            return Ok(());
        }
        println!();
    }

    // Perform uninstallation
    let mut errors = Vec::new();

    // Remove Studio plugin first (doesn't need elevated permissions)
    if let Some(plugins_folder) = get_studio_plugins_folder() {
        let plugin_path = plugins_folder.join("RbxSync.rbxm");
        if plugin_path.exists() {
            match std::fs::remove_file(&plugin_path) {
                Ok(()) => println!("Removed Studio plugin: {}", plugin_path.display()),
                Err(e) => errors.push(format!("Failed to remove plugin: {}", e)),
            }
        }
    }

    // Remove ~/.rbxsync directory
    if !keep_repo {
        if let Some(ref dir) = rbxsync_dir {
            if dir.exists() {
                match std::fs::remove_dir_all(dir) {
                    Ok(()) => println!("Removed data directory: {}", dir.display()),
                    Err(e) => errors.push(format!("Failed to remove ~/.rbxsync: {}", e)),
                }
            }
        }
    }

    // Remove VS Code extension
    if vscode {
        println!("Uninstalling VS Code extension...");

        #[cfg(target_os = "macos")]
        let code_cmd = "code";
        #[cfg(target_os = "windows")]
        let code_cmd = "code.cmd";
        #[cfg(target_os = "linux")]
        let code_cmd = "code";

        match std::process::Command::new(code_cmd)
            .args(["--uninstall-extension", vscode_extension_id])
            .status()
        {
            Ok(status) if status.success() => {
                println!("Removed VS Code extension: {}", vscode_extension_id);
            }
            Ok(_) => {
                errors.push("VS Code extension uninstall failed (may not be installed)".to_string());
            }
            Err(e) => {
                errors.push(format!("Could not run 'code' command: {}. Uninstall manually from VS Code.", e));
            }
        }
    }

    // Remove CLI binary last (this is what we're running!)
    if let Some(ref exe) = current_exe {
        if !exe.to_string_lossy().contains("target") {
            println!();
            println!("Removing CLI binary...");

            #[cfg(unix)]
            {
                // Try to remove directly first
                if std::fs::remove_file(exe).is_err() {
                    // Need elevated permissions
                    match std::process::Command::new("sudo")
                        .args(["rm", exe.to_str().unwrap()])
                        .status()
                    {
                        Ok(status) if status.success() => {
                            println!("Removed CLI: {}", exe.display());
                        }
                        _ => {
                            errors.push(format!(
                                "Could not remove CLI binary. Run manually:\n  sudo rm {}",
                                exe.display()
                            ));
                        }
                    }
                } else {
                    println!("Removed CLI: {}", exe.display());
                }
            }

            #[cfg(windows)]
            {
                // On Windows, we can't delete ourselves while running
                // Create a batch file to delete after exit
                let batch_path = std::env::temp_dir().join("rbxsync_uninstall.bat");
                let batch_content = format!(
                    "@echo off\n\
                    timeout /t 1 /nobreak > nul\n\
                    del /f /q \"{}\"\n\
                    del /f /q \"%~f0\"\n",
                    exe.display()
                );

                if std::fs::write(&batch_path, batch_content).is_ok() {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "/min", batch_path.to_str().unwrap()])
                        .spawn();
                    println!("CLI will be removed after exit.");
                } else {
                    errors.push(format!(
                        "Could not remove CLI binary. Delete manually:\n  del \"{}\"",
                        exe.display()
                    ));
                }
            }
        }
    }

    println!();

    if errors.is_empty() {
        println!("RbxSync has been uninstalled successfully!");
        println!();
        println!("Thanks for using RbxSync! If you have feedback, please share at:");
        println!("  https://github.com/devmarissa/rbxsync/issues");
    } else {
        println!("Uninstall completed with some issues:");
        for err in &errors {
            println!("  - {}", err);
        }
    }

    Ok(())
}

/// Migrate from Rojo project to RbxSync
fn cmd_migrate(from: String, path: Option<PathBuf>, force: bool) -> Result<()> {
    let project_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());

    println!("RbxSync Migration Tool");
    println!("======================");
    println!();

    match from.to_lowercase().as_str() {
        "rojo" => {
            // Find Rojo project file
            let rojo_path = match find_rojo_project(&project_dir) {
                Ok(path) => path,
                Err(e) => {
                    bail!(
                        "No Rojo project file found in {}.\n\
                        Expected: default.project.json or *.project.json\n\
                        Error: {}",
                        project_dir.display(),
                        e
                    );
                }
            };

            println!("Found Rojo project: {}", rojo_path.display());
            println!();

            // Parse Rojo config
            let rojo = parse_rojo_project(&rojo_path).context("Failed to parse Rojo project")?;

            println!("Project name: {}", rojo.name);
            println!();

            // Convert to RbxSync tree_mapping
            let tree_mapping = rojo_to_tree_mapping(&rojo);

            if tree_mapping.is_empty() {
                println!("Warning: No path mappings found in Rojo project.");
                println!("The Rojo project may use inline definitions without $path.");
            } else {
                println!("Detected directory mappings:");
                let mut sorted_mappings: Vec<_> = tree_mapping.iter().collect();
                sorted_mappings.sort_by(|a, b| a.0.cmp(b.0));
                for (datamodel_path, fs_path) in &sorted_mappings {
                    println!("  {} -> {}", datamodel_path, fs_path);
                }
                println!();
            }

            // Check for existing rbxsync.json
            let rbxsync_path = project_dir.join("rbxsync.json");
            if rbxsync_path.exists() && !force {
                bail!(
                    "rbxsync.json already exists at {}.\n\
                    Use --force to overwrite.",
                    rbxsync_path.display()
                );
            }

            // Determine source directory from Rojo config
            let source_dir = rbxsync_core::rojo::get_source_dir(&rojo)
                .unwrap_or_else(|| "src".to_string());

            // Create RbxSync config
            let rbxsync_config = ProjectConfig {
                name: rojo.name.clone(),
                tree: PathBuf::from(format!("./{}", source_dir)),
                tree_mapping,
                ..Default::default()
            };

            // Write rbxsync.json
            let json = serde_json::to_string_pretty(&rbxsync_config)?;
            std::fs::write(&rbxsync_path, &json).context("Failed to write rbxsync.json")?;

            println!("Created: {}", rbxsync_path.display());
            println!();

            // Show the generated config
            println!("Generated rbxsync.json:");
            println!("{}", json);
            println!();

            println!("Migration complete!");
            println!();
            println!("Next steps:");
            println!("  1. Review rbxsync.json and adjust settings if needed");
            println!("  2. Start the sync server: rbxsync serve");
            println!("  3. Connect from Roblox Studio with the RbxSync plugin");
            println!();
            println!("Note: Your existing Rojo project file was not modified.");
            println!("You can keep using both tools side-by-side if desired.");
        }
        other => {
            bail!(
                "Unknown source format: '{}'\n\
                Supported formats:\n\
                  - rojo: Migrate from Rojo project (default.project.json)",
                other
            );
        }
    }

    Ok(())
}

/// Manage AI development harness
async fn cmd_harness(action: HarnessAction) -> Result<()> {
    let client = reqwest::Client::new();

    // Check server is running
    if client.get("http://localhost:44755/health").send().await.is_err() {
        println!("RbxSync server is not running. Start it with: rbxsync serve");
        return Ok(());
    }

    match action {
        HarnessAction::Init {
            name,
            genre,
            description,
            path,
        } => {
            let project_dir = path
                .unwrap_or_else(|| std::env::current_dir().unwrap())
                .to_string_lossy()
                .to_string();

            println!("Initializing harness for project: {}", project_dir);

            let mut body = serde_json::json!({
                "projectDir": project_dir,
                "gameName": name,
            });

            if let Some(g) = genre {
                body["genre"] = serde_json::Value::String(g);
            }
            if let Some(d) = description {
                body["description"] = serde_json::Value::String(d);
            }

            let response = client
                .post("http://localhost:44755/harness/init")
                .json(&body)
                .send()
                .await
                .context("Failed to initialize harness")?;

            let result: serde_json::Value = response.json().await?;
            if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                let harness_dir = result
                    .get("harnessDir")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let game_id = result
                    .get("gameId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                println!("Harness initialized successfully!");
                println!("  Directory: {}", harness_dir);
                println!("  Game ID: {}", game_id);
            } else {
                let error = result
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                println!("Failed to initialize harness: {}", error);
            }
        }

        HarnessAction::Status { path } => {
            let project_dir = path
                .unwrap_or_else(|| std::env::current_dir().unwrap())
                .to_string_lossy()
                .to_string();

            let response = client
                .post("http://localhost:44755/harness/status")
                .json(&serde_json::json!({
                    "projectDir": project_dir,
                }))
                .send()
                .await
                .context("Failed to get harness status")?;

            let result: serde_json::Value = response.json().await?;

            if !result.get("initialized").and_then(|v| v.as_bool()).unwrap_or(false) {
                println!("Harness not initialized for this project.");
                println!("Run: rbxsync harness init --name 'Your Game'");
                return Ok(());
            }

            // Print game info
            if let Some(game) = result.get("game") {
                println!("Game: {}", game.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown"));
                if let Some(genre) = game.get("genre").and_then(|v| v.as_str()) {
                    println!("Genre: {}", genre);
                }
                if let Some(desc) = game.get("description").and_then(|v| v.as_str()) {
                    if !desc.is_empty() {
                        println!("Description: {}", desc);
                    }
                }
                println!();
            }

            // Print feature summary
            if let Some(summary) = result.get("featureSummary") {
                println!("Features:");
                let total = summary.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
                let planned = summary.get("planned").and_then(|v| v.as_u64()).unwrap_or(0);
                let in_progress = summary.get("inProgress").and_then(|v| v.as_u64()).unwrap_or(0);
                let completed = summary.get("completed").and_then(|v| v.as_u64()).unwrap_or(0);
                let blocked = summary.get("blocked").and_then(|v| v.as_u64()).unwrap_or(0);

                println!("  Total: {}", total);
                if planned > 0 {
                    println!("  Planned: {}", planned);
                }
                if in_progress > 0 {
                    println!("  In Progress: {}", in_progress);
                }
                if completed > 0 {
                    println!("  Completed: {}", completed);
                }
                if blocked > 0 {
                    println!("  Blocked: {}", blocked);
                }
                println!();
            }

            // Print recent sessions
            if let Some(sessions) = result.get("recentSessions").and_then(|v| v.as_array()) {
                if !sessions.is_empty() {
                    println!("Recent Sessions:");
                    for session in sessions.iter().take(3) {
                        let id = session.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let started = session.get("startedAt").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let ended = session.get("endedAt").and_then(|v| v.as_str());
                        let status = if ended.is_some() { "completed" } else { "active" };
                        println!("  {} ({}) - {}", &id[..8.min(id.len())], status, started);
                    }
                }
            }
        }

        HarnessAction::Features { status, path } => {
            let project_dir = path
                .unwrap_or_else(|| std::env::current_dir().unwrap())
                .to_string_lossy()
                .to_string();

            let response = client
                .post("http://localhost:44755/harness/status")
                .json(&serde_json::json!({
                    "projectDir": project_dir,
                }))
                .send()
                .await
                .context("Failed to get features")?;

            let result: serde_json::Value = response.json().await?;

            if !result.get("initialized").and_then(|v| v.as_bool()).unwrap_or(false) {
                println!("Harness not initialized for this project.");
                return Ok(());
            }

            let features = result.get("features").and_then(|v| v.as_array());
            if let Some(features) = features {
                if features.is_empty() {
                    println!("No features found.");
                    println!("Add one with: rbxsync harness feature 'Feature Name'");
                    return Ok(());
                }

                // Filter by status if provided
                let status_filter = status.as_ref().map(|s| s.to_lowercase());

                println!("Features:");
                println!("{:<36} {:<12} {:<8} {}", "ID", "Status", "Priority", "Name");
                println!("{}", "-".repeat(80));

                for feature in features {
                    let feature_status = feature
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_lowercase();

                    // Skip if doesn't match filter
                    if let Some(ref filter) = status_filter {
                        // Handle snake_case vs camelCase
                        let normalized_filter = filter.replace("_", "").replace("-", "");
                        let normalized_status = feature_status.replace("_", "").replace("-", "");
                        if !normalized_status.contains(&normalized_filter) {
                            continue;
                        }
                    }

                    let id = feature.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let name = feature.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed");
                    let priority = feature
                        .get("priority")
                        .and_then(|v| v.as_str())
                        .unwrap_or("medium");

                    println!("{:<36} {:<12} {:<8} {}", id, feature_status, priority, name);
                }
            } else {
                println!("No features found.");
            }
        }

        HarnessAction::Feature {
            name,
            id,
            status,
            description,
            priority,
            note,
            path,
        } => {
            let project_dir = path
                .unwrap_or_else(|| std::env::current_dir().unwrap())
                .to_string_lossy()
                .to_string();

            let mut body = serde_json::json!({
                "projectDir": project_dir,
            });

            if let Some(feature_id) = id {
                // Updating existing feature
                body["featureId"] = serde_json::Value::String(feature_id);
                body["name"] = serde_json::Value::String(name);
            } else {
                // Creating new feature
                body["name"] = serde_json::Value::String(name.clone());
            }

            if let Some(s) = status {
                // Convert CLI status format to API format
                let lower = s.to_lowercase();
                let api_status = match lower.as_str() {
                    "planned" => "planned",
                    "in_progress" | "inprogress" | "in-progress" => "in_progress",
                    "completed" | "done" => "completed",
                    "blocked" => "blocked",
                    "cancelled" | "canceled" => "cancelled",
                    _ => lower.as_str(),
                };
                body["status"] = serde_json::Value::String(api_status.to_string());
            }

            if let Some(d) = description {
                body["description"] = serde_json::Value::String(d);
            }

            if let Some(p) = priority {
                body["priority"] = serde_json::Value::String(p);
            }

            if let Some(n) = note {
                body["addNote"] = serde_json::Value::String(n);
            }

            let response = client
                .post("http://localhost:44755/harness/feature/update")
                .json(&body)
                .send()
                .await
                .context("Failed to update feature")?;

            let result: serde_json::Value = response.json().await?;
            if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                let feature_id = result
                    .get("featureId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let message = result
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Feature updated");

                println!("{}", message);
                println!("Feature ID: {}", feature_id);
            } else {
                let error = result
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                println!("Failed to update feature: {}", error);
            }
        }

        HarnessAction::Session { action } => match action {
            SessionAction::Start { goals, path } => {
                let project_dir = path
                    .unwrap_or_else(|| std::env::current_dir().unwrap())
                    .to_string_lossy()
                    .to_string();

                let mut body = serde_json::json!({
                    "projectDir": project_dir,
                });

                if let Some(g) = goals {
                    body["initialGoals"] = serde_json::Value::String(g);
                }

                let response = client
                    .post("http://localhost:44755/harness/session/start")
                    .json(&body)
                    .send()
                    .await
                    .context("Failed to start session")?;

                let result: serde_json::Value = response.json().await?;
                if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let session_id = result
                        .get("sessionId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    println!("Session started successfully!");
                    println!("Session ID: {}", session_id);
                    println!();
                    println!("When finished, end with:");
                    println!("  rbxsync harness session end --id {}", session_id);
                } else {
                    let error = result
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error");
                    println!("Failed to start session: {}", error);
                }
            }

            SessionAction::End {
                id,
                summary,
                handoff,
                path,
            } => {
                let project_dir = path
                    .unwrap_or_else(|| std::env::current_dir().unwrap())
                    .to_string_lossy()
                    .to_string();

                let mut body = serde_json::json!({
                    "projectDir": project_dir,
                    "sessionId": id,
                });

                if let Some(s) = summary {
                    body["summary"] = serde_json::Value::String(s);
                }

                if let Some(h) = handoff {
                    body["handoffNotes"] = serde_json::Value::Array(
                        h.into_iter().map(serde_json::Value::String).collect(),
                    );
                }

                let response = client
                    .post("http://localhost:44755/harness/session/end")
                    .json(&body)
                    .send()
                    .await
                    .context("Failed to end session")?;

                let result: serde_json::Value = response.json().await?;
                if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                    println!("Session ended successfully!");
                } else {
                    let error = result
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error");
                    println!("Failed to end session: {}", error);
                }
            }
        },
    }

    Ok(())
}
