# RbxSync

A professional tool for syncing Roblox games between Studio and VS Code with full property preservation, git integration, and AI-powered development workflows.

## Features

- **Complete DataModel Extraction**: Captures ALL properties using API dump reflection
- **Two-Way Sync**: Push changes from VS Code to Studio and extract from Studio to files
- **VS Code Extension**: Native integration with status bar, activity panel, and keyboard shortcuts
- **MCP Integration**: AI agents can extract, sync, run code, and test games
- **Git-Friendly**: Clean file structure designed for version control
- **Multi-Studio Support**: Work with multiple Studio instances simultaneously
- **Play Testing**: Run automated tests with console output capture

## Project Structure

```
rbxsync/
├── rbxsync-core/     # Core Rust library (types, serialization)
├── rbxsync-server/   # HTTP server (communicates with Studio plugin)
├── rbxsync-cli/      # CLI tool (init, extract, sync, serve)
├── rbxsync-mcp/      # MCP server for AI integration
├── rbxsync-vscode/   # VS Code extension
└── plugin/           # Roblox Studio plugin (Luau)
```

## Quick Start

### Installation

```bash
# Build everything
cargo build --release

# Install CLI globally
cp target/release/rbxsync /usr/local/bin/

# Build and install Studio plugin
./target/release/rbxsync build-plugin --install
```

### Initialize a Project

```bash
rbxsync init --name MyGame
```

Creates:
```
MyGame/
├── rbxsync.json      # Project configuration
├── src/              # Instance tree
│   ├── Workspace/
│   ├── ReplicatedStorage/
│   ├── ServerScriptService/
│   └── ...
├── assets/           # Binary assets
└── terrain/          # Terrain voxel data
```

### VS Code Extension

1. Build the extension:
   ```bash
   cd rbxsync-vscode
   npm install
   npm run build
   npx vsce package
   ```

2. Install the `.vsix` file in VS Code

3. Open your project folder in VS Code
4. Click the RbxSync icon in the activity bar
5. Click "Start Server"

### Studio Plugin

1. Open Roblox Studio
2. The RbxSync plugin widget appears
3. Set the project path to your VS Code workspace
4. Click "Connect"

## CLI Commands

```bash
rbxsync init [--name NAME]           # Initialize new project
rbxsync serve [--port PORT]          # Start sync server (default: 44755)
rbxsync stop                         # Stop the server
rbxsync status                       # Show connection status
rbxsync extract                      # Extract game from Studio
rbxsync sync [--path DIR]            # Sync local changes to Studio
rbxsync build-plugin [--install]     # Build Studio plugin
```

## Keyboard Shortcuts (VS Code)

| Shortcut | Command |
|----------|---------|
| `Cmd+Shift+S` | Sync to Studio |
| `Cmd+Shift+E` | Extract from Studio |
| `Cmd+Shift+T` | Run Play Test |
| `Cmd+Shift+M` | Open .rbxjson metadata |

## MCP Integration

RbxSync includes an MCP server for AI agent integration:

```bash
# Run the MCP server
./target/release/rbxsync-mcp
```

### Available Tools

| Tool | Description |
|------|-------------|
| `extract_game` | Extract game to files |
| `sync_to_studio` | Push local changes to Studio |
| `run_code` | Execute Luau code in Studio |
| `run_test` | Run play test with output capture |
| `git_status` | Get project git status |
| `git_commit` | Commit changes |

### Claude Desktop Config

```json
{
  "mcpServers": {
    "rbxsync": {
      "command": "/path/to/rbxsync-mcp"
    }
  }
}
```

## File Format

### Script Files (`.luau` / `.lua`)

Scripts are stored as plain Luau files:

```lua
-- src/ServerScriptService/Main.server.luau
local Players = game:GetService("Players")

Players.PlayerAdded:Connect(function(player)
    print("Welcome", player.Name)
end)
```

### Metadata Files (`.rbxjson`)

Instance properties are stored alongside scripts:

```json
{
  "className": "Script",
  "name": "Main",
  "properties": {
    "Disabled": false,
    "RunContext": "Server"
  },
  "attributes": {},
  "tags": []
}
```

### Non-Script Instances

Folders and other instances use `init.rbxjson`:

```json
{
  "className": "Folder",
  "name": "Modules",
  "properties": {},
  "attributes": {
    "Version": 1
  },
  "tags": ["Important"]
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│           VS Code Extension / CLI / MCP Server              │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                    Rust Server (port 44755)                  │
│  • Long-polling for commands                                 │
│  • Chunked extraction handling                               │
│  • Git operations                                            │
│  • Multi-workspace routing                                   │
└──────────────────────────┬──────────────────────────────────┘
                           │ HTTP (localhost)
┌──────────────────────────▼──────────────────────────────────┐
│                    Studio Plugin (Luau)                      │
│  • API dump reflection                                       │
│  • Instance serialization                                    │
│  • Console output capture                                    │
│  • Play test automation                                      │
└─────────────────────────────────────────────────────────────┘
```

## Development

### Building

```bash
# Build all Rust packages
cargo build --release

# Build VS Code extension
cd rbxsync-vscode && npm run build

# Build Studio plugin
rojo build plugin/default.project.json -o build/RbxSync.rbxm
```

### Testing

```bash
# Run Rust tests
cargo test

# Run with debug logging
RUST_LOG=debug ./target/release/rbxsync serve
```

## License

MIT
