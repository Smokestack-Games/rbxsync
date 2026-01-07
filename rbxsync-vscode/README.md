# RbxSync for VS Code

Extract and sync Roblox games to git-friendly files, directly from VS Code.

## Features

- **Connect to Studio** - One-click connection to your Roblox Studio instance
- **Extract Game** - Pull your entire game into version-controlled files
- **Sync Changes** - Push local edits back to Studio instantly
- **Instance Explorer** - Browse your game structure in the sidebar
- **Git Integration** - View staged/modified files and commit directly

## Requirements

1. **RbxSync Server** - Install and run the `rbxsync` CLI:
   ```bash
   cargo install rbxsync
   rbxsync serve
   ```

2. **RbxSync Studio Plugin** - Purchase and install the plugin from [rbxsync.dev](https://rbxsync.dev)

## Getting Started

1. Open a folder containing an `rbxsync.json` project file
2. Start the RbxSync server: `rbxsync serve`
3. Open Roblox Studio with the RbxSync plugin enabled
4. The extension will auto-connect (or click the status bar to connect)
5. Use the RbxSync sidebar to extract and sync your game

## Commands

| Command | Description |
|---------|-------------|
| `RbxSync: Connect to Studio` | Connect to the RbxSync server |
| `RbxSync: Extract Game from Studio` | Extract game to local files |
| `RbxSync: Sync Changes to Studio` | Push local changes to Studio |
| `RbxSync: Show Git Status` | Display git repository status |
| `RbxSync: Commit Changes` | Commit staged changes |

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `rbxsync.serverPort` | `44755` | RbxSync server port |
| `rbxsync.autoConnect` | `true` | Auto-connect on startup |
| `rbxsync.showNotifications` | `true` | Show operation notifications |

## File Structure

```
my-game/
├── rbxsync.json          # Project config
└── src/
    ├── ServerScriptService/
    │   └── Main.server.luau
    ├── ReplicatedStorage/
    │   └── Modules/
    └── Workspace/
        └── Models/
```

## License

The VS Code extension is free. The Roblox Studio plugin requires a license.
