# CLI Commands

Complete reference for all RbxSync CLI commands.

## Core Commands

### init
Initialize a new RbxSync project.

```bash
rbxsync init [--name NAME]
```

Creates `rbxsync.json` and the `src/` directory structure.

### serve
Start the sync server.

```bash
rbxsync serve [--port PORT] [--background]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--port` | 44755 | Server port |
| `--background, -b` | false | Run server as a background daemon |

Run in background mode for a cleaner terminal:

```bash
# Start in background
rbxsync serve --background

# Stop the background server
rbxsync stop
```

### stop
Stop the running server.

```bash
rbxsync stop
```

### status
Show connection status.

```bash
rbxsync status
```

### extract
Extract game from connected Studio to files.

```bash
rbxsync extract
```

Requires an active Studio connection.

### sync
Push local changes to Studio.

```bash
rbxsync sync [--path DIR]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--path` | Current dir | Project path |

## Build Commands

### build
Build project to Roblox format.

```bash
rbxsync build [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `-f, --format` | rbxl | Output format: rbxl, rbxm, rbxlx, rbxmx |
| `-o, --output` | build/ | Output path |
| `--watch` | false | Watch for changes and rebuild |
| `--plugin` | - | Build directly to Studio plugins folder |

Examples:

```bash
# Build place file
rbxsync build

# Build model file
rbxsync build -f rbxm

# Build XML format
rbxsync build -f rbxlx

# Watch mode
rbxsync build --watch

# Build as plugin
rbxsync build --plugin MyPlugin.rbxm
```

### build-plugin
Build the RbxSync Studio plugin.

```bash
rbxsync build-plugin [--install]
```

| Option | Description |
|--------|-------------|
| `--install` | Copy to Studio plugins folder |

## Utility Commands

### sourcemap
Generate `sourcemap.json` for Luau LSP.

```bash
rbxsync sourcemap
```

The VS Code extension already generates `default.project.json` from
`datamodel.rbxjson` for language-server support, so most projects don't need
this. Use it when a tool specifically wants a `sourcemap.json`.

### fmt-project
Format `datamodel.rbxjson` (and any other `.rbxjson` files, such as terrain data)
with consistent style.

```bash
rbxsync fmt-project [--check]
```

| Option | Description |
|--------|-------------|
| `--check` | Check only, don't modify (for CI) |

### studio
Launch Roblox Studio.

```bash
rbxsync studio [file.rbxl]
```

### doc
Open documentation in browser.

```bash
rbxsync doc
```

## Update Commands

### version
Show version and git commit.

```bash
rbxsync version
```

### update
Pull latest changes and rebuild.

```bash
rbxsync update [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--from-source` | Build from source instead of downloading a release |
| `--vscode` | Also update the VS Code extension |
| `-y, --yes` | Skip the confirmation prompt |

This command:
1. Pulls latest from GitHub
2. Rebuilds the CLI
3. Rebuilds and installs the Studio plugin

Then restart Studio to load the updated plugin.

### uninstall
Completely remove RbxSync from your system.

```bash
rbxsync uninstall [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--vscode` | Also remove VS Code extension |
| `--keep-repo` | Keep the cloned repo at ~/.rbxsync/repo |
| `-y, --yes` | Skip confirmation prompt |

## Migration Commands

### migrate
Migrate from another sync tool to RbxSync.

```bash
rbxsync migrate [--from FORMAT] [--path DIR] [--force]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--from` | rojo | Source format to migrate from |
| `--path` | Current dir | Project directory |
| `--force` | false | Overwrite existing rbxsync.json |

Currently supports migrating from Rojo projects.

Example:

```bash
# Migrate a Rojo project
cd my-rojo-project
rbxsync migrate

# Or specify the path
rbxsync migrate --path /path/to/rojo/project

# Force overwrite existing config
rbxsync migrate --force
```

This reads your `default.project.json` (or `*.project.json`) and creates an equivalent `rbxsync.json` with:
- Project name
- Tree mappings (DataModel path → filesystem path)
- Default RbxSync settings

Your Rojo project file is preserved—you can use both tools side-by-side.
