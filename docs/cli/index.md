# CLI Overview

The RbxSync CLI is your primary interface for project management, syncing, and building.

## Installation

```bash
cargo build --release
cp target/release/rbxsync /usr/local/bin/
```

## Basic Usage

```bash
# Initialize a new project
rbxsync init --name MyGame

# Start the sync server
rbxsync serve

# Check connection status
rbxsync status

# Extract game from Studio
rbxsync extract

# Sync local changes to Studio
rbxsync sync
```

## Command Categories

### Core Commands
Essential commands for daily workflow.
- `init` - Create new project
- `serve` - Start sync server
- `stop` - Stop server
- `extract` - Pull game from Studio
- `sync` - Push changes to Studio

### Build Commands
Compile your project to Roblox formats.
- `build` - Build to .rbxl, .rbxm, etc.
- `build-plugin` - Build Studio plugin

### Utility Commands
Helpful tools for development.
- `sourcemap` - Generate a `sourcemap.json` for Luau LSP
- `fmt-project` - Format `datamodel.rbxjson` and other `.rbxjson` files
- `studio` - Launch Roblox Studio
- `doc` - Open documentation

### Update Commands
Keep RbxSync current.
- `version` - Show version info
- `update` - Pull latest and rebuild

See [Commands](/cli/commands) for the full reference.
