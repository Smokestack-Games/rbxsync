# RbxSync

A professional tool for syncing Roblox games between Studio and VS Code with full property preservation, git integration, and AI-powered development workflows.

## Why RbxSync?

- **No More Workplace Discrepancies**: Everyone syncs from Git. No more "which version is the latest?"
- **Full Property Extraction**: Captures ALL properties (not just scripts) using API dump reflection
- **Two-Way Sync**: Changes in Studio automatically sync back to files
- **AI-Powered Development**: MCP integration lets AI agents write, test, and debug code

## Feature Comparison

| Feature | RbxSync | Rojo | Argon |
|---------|---------|------|-------|
| Two-way sync | ✅ | ❌ | ✅ |
| Full property extraction | ✅ | ❌ | ◐ |
| Build to .rbxl/.rbxm | ✅ | ✅ | ✅ |
| Build --watch mode | ✅ | ✅ | ✅ |
| Build --plugin flag | ✅ | ✅ | ❌ |
| XML format support | ✅ | ✅ | ✅ |
| MCP / AI integration | ✅ | ❌ | ❌ |
| E2E testing mode | ✅ | ❌ | ❌ |
| Console streaming | ✅ | ❌ | ❌ |

## Installation

### 1. Build from Source

```bash
git clone https://github.com/devmarissa/rbxsync
cd rbxsync
cargo build --release

# Add to PATH (optional)
cp target/release/rbxsync /usr/local/bin/
```

### 2. Install Studio Plugin

```bash
rbxsync build-plugin --install
```

The plugin will be installed to `~/Documents/Roblox/Plugins/RbxSync.rbxm`

### 3. Install VS Code Extension (Optional)

```bash
cd rbxsync-vscode
npm install
npm run build
npm run package

# Install the extension (pick one method):

# Option 1: Command line
code --install-extension rbxsync-1.0.0.vsix

# Option 2: VS Code UI
# 1. Press Cmd+Shift+P (Mac) or Ctrl+Shift+P (Win)
# 2. Type "Install from VSIX"
# 3. Select rbxsync-1.0.0.vsix
```

### 4. Initialize Project & Connect

```bash
rbxsync init --name MyGame
rbxsync serve
```

Then in Roblox Studio:
1. Restart Studio to load the plugin
2. Set the project path in the RbxSync widget
3. Click "Connect"

## CLI Commands

### Core Commands

```bash
rbxsync init [--name NAME]           # Initialize new project
rbxsync serve [--port PORT]          # Start sync server (default: 44755)
rbxsync stop                         # Stop the server
rbxsync status                       # Show connection status
rbxsync extract                      # Extract game from Studio
rbxsync sync [--path DIR]            # Sync local changes to Studio
```

### Build Commands

```bash
rbxsync build                        # Build project to .rbxl (place)
rbxsync build -f rbxm                # Build to .rbxm (model)
rbxsync build -f rbxlx               # Build to .rbxlx (XML place)
rbxsync build -f rbxmx               # Build to .rbxmx (XML model)
rbxsync build --watch                # Watch for changes and auto-rebuild
rbxsync build --plugin MyPlugin.rbxm # Build directly to Studio plugins folder
rbxsync build -o output.rbxl         # Specify output path
```

### Utility Commands

```bash
rbxsync build-plugin [--install]     # Build Studio plugin from source
rbxsync sourcemap                    # Generate sourcemap.json for Luau LSP
rbxsync fmt-project                  # Format all .rbxjson files
rbxsync fmt-project --check          # Check formatting (for CI)
rbxsync doc                          # Open documentation in browser
rbxsync studio [file.rbxl]           # Launch Roblox Studio
```

### Update Commands

```bash
rbxsync version                      # Show version and git commit
rbxsync update                       # Pull latest, rebuild CLI + plugin
rbxsync update --vscode              # Also rebuild VS Code extension
rbxsync update --no-pull             # Just rebuild (skip git pull)
```

## Updating

When new fixes are released, update with a single command:

```bash
rbxsync update
```

This will:
1. Pull the latest changes from GitHub
2. Rebuild the CLI
3. Rebuild and install the Studio plugin

Then restart Roblox Studio to load the updated plugin.

**For VS Code extension updates:**
```bash
rbxsync update --vscode
code --install-extension rbxsync-vscode/rbxsync-1.0.0.vsix
```
Then restart VS Code.

## File Format

### Script Files (`.luau`)

Scripts are stored as plain Luau files with naming conventions:

```
MyScript.server.luau  → Script (runs on server)
MyScript.client.luau  → LocalScript (runs on client)
MyScript.luau         → ModuleScript
```

Example:
```lua
-- src/ServerScriptService/Main.server.luau
local Players = game:GetService("Players")

Players.PlayerAdded:Connect(function(player)
    print("Welcome", player.Name)
end)
```

### Instance Files (`.rbxjson`)

Non-script instances are stored as `.rbxjson` files with full property preservation:

```json
{
  "className": "Part",
  "name": "Baseplate",
  "properties": {
    "Anchored": {
      "type": "bool",
      "value": true
    },
    "Size": {
      "type": "Vector3",
      "value": { "x": 512, "y": 20, "z": 512 }
    },
    "Color": {
      "type": "Color3",
      "value": { "r": 0.388, "g": 0.372, "b": 0.384 }
    },
    "Material": {
      "type": "Enum",
      "value": { "enumType": "Material", "value": "Grass" }
    }
  }
}
```

### Supported Property Types

| Type | Example Value |
|------|---------------|
| `string` | `"Hello"` |
| `bool` | `true` / `false` |
| `int` / `int32` / `int64` | `42` |
| `float` / `float32` / `float64` | `3.14` |
| `Vector2` | `{ "x": 0, "y": 0 }` |
| `Vector3` | `{ "x": 0, "y": 0, "z": 0 }` |
| `CFrame` | `{ "position": [0,0,0], "rotation": [1,0,0,0,1,0,0,0,1] }` |
| `Color3` | `{ "r": 1, "g": 0.5, "b": 0 }` |
| `Color3uint8` | `{ "r": 255, "g": 128, "b": 0 }` |
| `BrickColor` | `194` (number) |
| `UDim` | `{ "scale": 0.5, "offset": 10 }` |
| `UDim2` | `{ "x": {...}, "y": {...} }` |
| `Rect` | `{ "min": {...}, "max": {...} }` |
| `NumberRange` | `{ "min": 0, "max": 100 }` |
| `Enum` | `{ "enumType": "Material", "value": "Plastic" }` |
| `Content` | `"rbxassetid://123456"` |
| `Font` | `{ "family": "...", "weight": 400, "style": "Normal" }` |

### Folder Meta Files (`_meta.rbxjson`)

Use `_meta.rbxjson` to set properties on folder instances:

```
src/
├── Workspace/
│   ├── _meta.rbxjson      # Properties for Workspace service
│   ├── Baseplate.rbxjson
│   └── SpawnLocation.rbxjson
```

## Project Structure

```
MyGame/
├── rbxsync.json          # Project configuration
├── src/                  # Instance tree
│   ├── Workspace/
│   ├── ReplicatedStorage/
│   ├── ServerScriptService/
│   ├── ServerStorage/
│   ├── StarterGui/
│   ├── StarterPack/
│   ├── StarterPlayer/
│   └── Lighting.rbxjson
├── build/                # Build output
└── sourcemap.json        # For Luau LSP
```

## MCP Integration (AI Agents)

RbxSync includes an MCP server for AI agent integration:

```bash
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

## E2E Testing Mode

AI agents can run playtests and see console output in real-time:

1. In VS Code, run command: `RbxSync: Toggle E2E Mode`
2. Open the console: `RbxSync: Open Console`
3. Studio `print()`, `warn()`, and `error()` output streams to the terminal

This enables AI to:
- Write code
- Run playtests
- See errors in real-time
- Debug and fix issues
- Iterate autonomously

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│           VS Code Extension / CLI / MCP Server              │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                    Rust Server (port 44755)                  │
│  • File watching with auto-sync                              │
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

## Troubleshooting

### Server won't start
- Check if port 44755 is already in use: `lsof -i :44755`
- Try stopping existing server: `rbxsync stop`

### Plugin not connecting
- Ensure the server is running: `rbxsync status`
- Check the project path in the plugin matches your VS Code workspace
- Enable HttpService in Roblox Studio (Game Settings > Security)

### Changes not syncing
- Verify the connection is established (green status in plugin)
- Check the VS Code output panel for errors
- Restart the server if needed

### Build fails with property errors
- Run `rbxsync fmt-project` to fix JSON formatting
- Check for unsupported property types in the error message

## Development

### Building

```bash
# Build all Rust packages
cargo build --release

# Build VS Code extension
cd rbxsync-vscode && npm run build && npm run package

# Build Studio plugin
rbxsync build-plugin
```

### Testing

```bash
# Run Rust tests
cargo test

# Run with debug logging
RUST_LOG=debug rbxsync serve
```

## License

MIT
