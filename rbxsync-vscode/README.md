# RbxSync for VS Code

Extract and sync Roblox games to git-friendly files, directly from VS Code.

## Features

- **Connect to Studio** - One-click connection to your Roblox Studio instance
- **Extract Game** - Pull your entire game into version-controlled files
- **Sync Changes** - Push local edits back to Studio instantly
- **Auto-Extract** - Changes in Studio automatically sync to files
- **Console Streaming** - View Studio console output in VS Code terminal
- **E2E Testing Mode** - AI-powered development with real-time feedback

## Requirements

1. **RbxSync Server** - Install and run the `rbxsync` CLI:
   ```bash
   cargo install rbxsync
   rbxsync serve
   ```

2. **RbxSync Studio Plugin** - Build from source or download from releases

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
| `RbxSync: Open Console` | Open Studio console output terminal |
| `RbxSync: Toggle E2E Mode` | Enable/disable E2E testing mode |
| `RbxSync: Show Git Status` | Display git repository status |
| `RbxSync: Commit Changes` | Commit staged changes |

## Console Streaming

The extension can stream Studio console output to a VS Code terminal:

1. Run `RbxSync: Open Console` to open the console terminal
2. All `print()`, `warn()`, and `error()` from Studio appear in real-time
3. Messages are color-coded: white (info), yellow (warn), red (error)

### E2E Testing Mode

When E2E mode is enabled, the console terminal auto-opens during operations - useful for AI-powered development workflows.

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

Copyright (c) 2026 Smokestack Games. All rights reserved. See LICENSE.txt for details.
