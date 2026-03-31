# RbxSync Studio Plugin

The Roblox Studio plugin component of RbxSync, enabling two-way sync between Studio and your file system.

## Features

- **Connect to Server** - Link to RbxSync server with one click
- **Extract Game** - Export all instances to version-controlled files
- **Sync from Files** - Push file changes to Studio
- **Auto-Extract** - Automatically extract changes made in Studio
- **Console Capture** - Stream print/warn/error to VS Code terminal

## Installation

### Option 1: Build from Source

```bash
# From the rbxsync root directory
rbxsync build-plugin --install
```

This builds `RbxSync.rbxm` and copies it to your Studio plugins folder.

### Option 2: Manual Install (using Rojo)

If you already have [Rojo](https://rojo.space) installed, you can build manually:

1. Build the plugin:
   ```bash
   rojo build plugin/default.project.json -o build/RbxSync.rbxm
   ```

2. Copy `build/RbxSync.rbxm` to:
   - **macOS**: `~/Documents/Roblox/Plugins/`
   - **Windows**: `%LOCALAPPDATA%\Roblox\Plugins\`

> **Note:** Option 1 (`rbxsync build-plugin --install`) is preferred — it handles building and installation in one step without requiring Rojo.

### Option 3: Creator Store

Coming soon - will be available on the Roblox Creator Store.

## Usage

### First Time Setup

1. Start the RbxSync server: `rbxsync serve`
2. Open Roblox Studio
3. Click the RbxSync button in the toolbar to open the panel
4. Enter your project path (e.g., `/Users/you/MyGame`)
5. Click **Connect**

### UI Overview

```
┌─────────────────────────┐
│  RBXSYNC               │
│  Studio ↔ VS Code      │
├─────────────────────────┤
│  ● Connected           │ <- Connection status
│  [Disconnect]          │
├─────────────────────────┤
│  PROJECT               │
│  /Users/you/MyGame  ▾  │ <- Project path dropdown
├─────────────────────────┤
│  [⬆ Extract] [⬇ Sync]  │ <- Main actions
├─────────────────────────┤
│  MyGame (12345)        │ <- Place info
│  1,247 instances       │
└─────────────────────────┘
```

### Connection States

| State | Indicator | Description |
|-------|-----------|-------------|
| Disconnected | Gray dot | Server not running or not connected |
| Connected | Green dot | Ready for sync operations |
| Syncing | Pulsing cyan | Operation in progress |

### Button States

**Sync Button:**
- Idle: Gray background, ready to sync
- Syncing: Cyan with pulse animation
- Success: Brief green flash

**Extract Button:**
- Active: Cyan when connected
- Disabled: Dimmed when disconnected

## Auto-Extract

When connected, changes you make in Studio are automatically extracted to files:

- Creating a new script
- Editing script source
- Deleting instances
- Modifying properties

Changes are debounced (300ms) to batch rapid edits.

### Tracked Services

Auto-extract monitors these services:
- Workspace
- ReplicatedStorage
- ReplicatedFirst
- ServerScriptService
- ServerStorage
- StarterGui
- StarterPack
- StarterPlayer
- Lighting
- SoundService

## Console Capture

When connected, all `print()`, `warn()`, and `error()` output is streamed to the VS Code console terminal.

This enables AI agents to:
- See test output in real-time
- Debug scripts remotely
- Verify changes worked

## File Structure

The plugin creates this file structure:

```
MyGame/
└── src/
    ├── ServerScriptService/
    │   ├── Main.server.luau    <- Script source
    │   └── Main.rbxjson        <- Script metadata
    ├── ReplicatedStorage/
    │   └── Modules/
    │       ├── _meta.rbxjson   <- Folder metadata
    │       └── Utils.luau
    └── Workspace/
        └── Parts/
            └── Part.rbxjson    <- Non-script instance
```

### File Types

| Extension | Content |
|-----------|---------|
| `.server.luau` | Server script source |
| `.client.luau` | Local script source |
| `.luau` | Module script source |
| `.rbxjson` | Instance properties/metadata |
| `_meta.rbxjson` | Container folder metadata |

## Troubleshooting

### "Not connected" error
- Make sure the server is running: `rbxsync serve`
- Check if HttpService is enabled in Game Settings > Security

### Changes not auto-extracting
- Verify the green connection dot is showing
- Check that changes are in tracked services
- Look at the VS Code output panel for errors

### "Can't connect to server"
- Server might not be running: `rbxsync serve`
- Port might be blocked: default is 44755
- Check firewall settings

### Sync taking too long
- Large games with thousands of instances may take time
- Progress is shown in the status area
- Consider extracting specific services only

## Development

### Building

```bash
# Recommended: using rbxsync CLI
rbxsync build-plugin

# Alternative: using Rojo directly (requires Rojo to be installed)
rojo build plugin/default.project.json -o build/RbxSync.rbxm
```

### Testing Changes

1. Make changes to `plugin/src/*.luau`
2. Rebuild: `rbxsync build-plugin --install`
3. Restart Roblox Studio to load the new plugin

### Debug Output

Enable verbose logging by checking the Output window in Studio for `[RbxSync]` messages.

## License

MIT
