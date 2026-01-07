//! RbxSync CLI
//!
//! Command-line interface for Roblox game extraction and synchronization.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rbxsync_core::{build_plugin, install_plugin, PluginBuildConfig, ProjectConfig};
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
        Commands::BuildPlugin {
            source,
            output,
            name,
            install,
        } => {
            cmd_build_plugin(source, output, name, install)?;
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
