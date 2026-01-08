# RbxSync

A professional tool for syncing Roblox games between Studio and VS Code with full property preservation, git integration, and AI-powered development workflows.

## Features

- **Complete DataModel Extraction**: Captures ALL properties using API dump reflection
- **Two-Way Sync**: Push changes from VS Code to Studio and auto-extract changes from Studio to files
- **VS Code Extension**: Native integration with status bar, activity panel, and keyboard shortcuts
- **E2E Testing Mode**: Stream Studio console output to VS Code for AI-powered development workflows
- **MCP Integration**: AI agents can extract, sync, run code, and test games
- **Git-Friendly**: Clean file structure designed for version control
- **Multi-Studio Support**: Work with multiple Studio instances simultaneously
- **Auto-Extract**: Changes made in Studio are automatically synced back to files

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

## E2E Testing Mode

RbxSync includes E2E testing mode for AI-powered development. When enabled, Studio console output streams to VS Code in real-time.

### Enable E2E Mode

1. In VS Code, run command: `RbxSync: Toggle E2E Mode`
2. Open the console: `RbxSync: Open Console`
3. Studio `print()`, `warn()`, and `error()` output will stream to the terminal

### Console Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /console/subscribe` | SSE stream of console messages |
| `GET /console/history` | Get recent console history |
| `POST /console/push` | Push messages (used by plugin) |

## Troubleshooting

### Server won't start
- Check if port 44755 is already in use: `lsof -i :44755`
- Try stopping existing server: `rbxsync stop`

### Plugin not connecting
- Ensure the server is running: `rbxsync status`
- Check the project path in the plugin matches your VS Code workspace
- Enable HttpService in Roblox Studio (Game Settings > Security)

### Changes not syncing
- Verify the connection is established (green dot in plugin)
- Check the VS Code output panel for errors
- Restart the server if needed

### Auto-extract not working
- Make sure you're connected (not just server running)
- Changes must be in tracked services (Workspace, ReplicatedStorage, etc.)

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
