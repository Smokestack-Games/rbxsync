# RbxSync

A commercial-grade tool for extracting complete Roblox games to git-friendly repositories with full property preservation.

## Features

- **Complete DataModel Extraction**: Captures ALL properties of every instance using API dump reflection
- **Custom JSON Format**: Human-readable `.rbxjson` files with proper type encoding
- **Terrain Support**: Extracts voxel data in compressed binary chunks
- **CSG/Union Handling**: Preserves asset references for mesh operations
- **Chunked Transfer**: Handles large games efficiently without memory issues
- **Git-Friendly**: Clean file structure designed for version control

## Project Structure

```
rbxsync/
├── rbxsync-core/     # Core Rust library (types, serialization)
├── rbxsync-server/   # HTTP server (communicates with Studio plugin)
├── rbxsync-cli/      # CLI tool (init, extract, sync)
└── plugin/           # Roblox Studio plugin (Luau)
```

## Quick Start

### 1. Build the CLI

```bash
cd rbxsync
cargo build --release
```

### 2. Initialize a Project

```bash
rbxsync init --name MyGame
```

This creates:
```
MyGame/
├── rbxsync.json      # Project configuration
├── src/              # Instance tree will be extracted here
│   ├── Workspace/
│   ├── ReplicatedStorage/
│   └── ...
├── assets/           # Binary assets (meshes, images, sounds)
└── terrain/          # Terrain voxel data
```

### 3. Install the Studio Plugin

Copy the `plugin/` folder contents to your Roblox Studio plugins directory.

### 4. Extract Your Game

1. Open your game in Roblox Studio
2. Enable the RbxSync plugin (click toolbar button)
3. Run: `rbxsync extract`

## File Format

### Instance JSON (`.rbxjson`)

```json
{
  "className": "Part",
  "name": "Baseplate",
  "referenceId": "uuid-v4",
  "properties": {
    "Anchored": { "type": "bool", "value": true },
    "Size": { "type": "Vector3", "value": { "x": 512, "y": 20, "z": 512 } },
    "Material": { "type": "Enum", "value": { "enumType": "Material", "value": "Plastic" } }
  },
  "attributes": {},
  "tags": ["Ground"]
}
```

### Project Config (`rbxsync.json`)

```json
{
  "name": "MyGame",
  "tree": "./src",
  "assets": "./assets",
  "config": {
    "extractBinaryAssets": true,
    "terrainMode": "voxelData",
    "excludeServices": ["CoreGui", "CorePackages"]
  }
}
```

## CLI Commands

```bash
rbxsync init                    # Initialize new project
rbxsync extract                 # Extract game from Studio
rbxsync extract --service Workspace  # Extract specific service
rbxsync serve                   # Start sync server
rbxsync status                  # Show connection status
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      CLI / VSCode Extension                  │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                    Rust Server (port 44755)                  │
│                    - HTTP endpoints                          │
│                    - Chunked transfer                        │
│                    - File system operations                  │
└──────────────────────────┬──────────────────────────────────┘
                           │ HTTP (localhost)
┌──────────────────────────▼──────────────────────────────────┐
│                    Studio Plugin (Luau)                      │
│                    - API dump reflection                     │
│                    - Instance serialization                  │
│                    - Terrain/CSG handlers                    │
└─────────────────────────────────────────────────────────────┘
```

## Property Type Support

RbxSync supports all 30+ Roblox property types:

| Type | Encoded As |
|------|-----------|
| `Vector3` | `{ "type": "Vector3", "value": { "x": 1, "y": 2, "z": 3 } }` |
| `CFrame` | Position + 3x3 rotation matrix |
| `Color3` | RGB values (0-1 range) |
| `Enum` | `{ "enumType": "Material", "value": "Plastic" }` |
| `Instance` (ref) | UUID reference |
| ... | See `rbxsync-core/src/types/properties.rs` |

## Roadmap

- [x] Core property type system
- [x] Studio plugin with reflection
- [x] CLI with init/extract commands
- [x] Terrain voxel extraction
- [ ] Two-way sync (push/pull)
- [ ] VSCode extension
- [ ] Commercial licensing

## License

MIT
