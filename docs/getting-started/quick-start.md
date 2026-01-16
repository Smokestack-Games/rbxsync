# Quick Start

Get your first project syncing in under 5 minutes.

## Initialize Project

```bash
rbxsync init --name MyGame
```

This creates a project structure:

```
MyGame/
├── rbxsync.json          # Project configuration
├── src/                  # Instance tree
│   ├── Workspace/
│   ├── ReplicatedStorage/
│   ├── ServerScriptService/
│   └── ...
├── assets/               # Binary assets (meshes, images, sounds)
└── terrain/              # Terrain voxel data
```

::: tip Luau LSP
Run `rbxsync sourcemap` to generate sourcemap.json for Luau language server support.
:::

## Start the Server

```bash
rbxsync serve
```

The server runs on port 44755 by default.

## Connect Studio

1. Open Roblox Studio
2. Restart Studio if you just installed the plugin
3. Click the RbxSync button in the toolbar
4. Enter your project path (e.g., `/Users/you/MyGame`)
5. Click **Connect**

You should see a green connection indicator.

## Extract Your Game

**Extraction** converts your Roblox game into local files that can be version-controlled with Git.

### What Gets Extracted

| In Studio | Becomes |
|-----------|---------|
| Scripts | `.luau` files (ServerScript → `.server.luau`, LocalScript → `.client.luau`) |
| Parts, Models, UI | `.rbxm` binary files |
| Properties | `.rbxjson` metadata files |
| Folders | Directories matching the hierarchy |
| Terrain | `terrain/` voxel data |

### How to Extract

1. Open your existing game in Studio
2. Start the server: `rbxsync serve`
3. Connect via the RbxSync plugin toolbar button
4. Click **Extract** in the plugin panel

Your game structure will appear in the `src/` folder:

```
src/
├── Workspace/
│   ├── Baseplate.rbxm
│   └── SpawnLocation.rbxm
├── ServerScriptService/
│   └── GameManager.server.luau
├── ReplicatedStorage/
│   └── Modules/
│       └── Utils.luau
└── StarterPlayer/
    └── StarterPlayerScripts/
        └── ClientInit.client.luau
```

### When to Extract

- **First time setup**: Convert an existing game to files
- **After Studio-only changes**: Pull in changes made directly in Studio
- **Team sync**: Get the latest from a teammate who worked in Studio

::: tip
Extraction also generates `project.json` for [Luau LSP](/vscode/luau-lsp), giving you intellisense in VS Code.
:::

## Sync Changes

Edit files in VS Code. Changes sync to Studio automatically when connected.

Or manually sync:

```bash
rbxsync sync
```

## What's Next?

- [CLI Commands](/cli/commands) - Full command reference
- [File Formats](/file-formats/) - Understand .luau and .rbxjson files
- [E2E Testing](/vscode/e2e-testing) - AI-powered development
