# Serialization

This document explains RbxSync's file format decisions, covering how Roblox instances are serialized to disk for version control and external editing.

## Design Philosophy

RbxSync prioritizes:

1. **Git-friendly formats** - Human-readable diffs, mergeable changes
2. **Full fidelity** - No data loss during round-trips between Studio and filesystem
3. **External editor support** - Scripts as plain Luau files for LSP integration
4. **Simplicity** - Minimal configuration, predictable file structure

## File Formats Overview

| Format | Extension | Purpose |
|--------|-----------|---------|
| Luau Scripts | `.luau` | Script source code (plain text) |
| Instance Data | `.rbxjson` | Properties and metadata (JSON) |
| Binary Model | `.rbxm` | Plugin distribution only |

### Why JSON Over Binary?

RbxSync uses `.rbxjson` (JSON) instead of binary formats like `.rbxm` for game data:

- **Readable diffs**: JSON changes are visible in Git diffs
- **Mergeable**: Concurrent changes can be merged manually
- **Editable**: Properties can be modified in any text editor
- **Type-safe**: Explicit type annotations prevent data corruption

Binary `.rbxm` files are only used for building the RbxSync Studio plugin itself.

## Script Files (.luau)

Scripts are stored as plain Luau files. The file extension determines the script type:

```
MyScript.server.luau  →  Script (runs on server)
MyScript.client.luau  →  LocalScript (runs on client)
MyScript.luau         →  ModuleScript
```

### Why Separate Script Source?

Script source is stored separately from instance metadata because:

1. **LSP support** - Editors like VS Code can provide autocomplete and diagnostics
2. **Readable diffs** - Code changes are clearly visible in Git history
3. **Single source of truth** - The `.luau` file is the canonical source

### Script Properties

If a script needs non-default properties (like `Enabled = false`), create a companion `.rbxjson`:

```
ServerScriptService/
├── GameLoop.server.luau     # Source code
└── GameLoop.rbxjson         # Properties (optional)
```

`GameLoop.rbxjson`:
```json
{
  "className": "Script",
  "properties": {
    "Enabled": { "type": "bool", "value": false },
    "RunContext": { "type": "Enum", "value": { "enumType": "RunContext", "value": "Server" } }
  }
}
```

See [.luau Scripts](/file-formats/luau) for more details.

## Instance Data (.rbxjson)

Non-script instances use `.rbxjson` files with explicit type annotations:

```json
{
  "className": "Part",
  "name": "Baseplate",
  "properties": {
    "Anchored": { "type": "bool", "value": true },
    "Size": { "type": "Vector3", "value": { "x": 512, "y": 20, "z": 512 } },
    "Material": { "type": "Enum", "value": { "enumType": "Material", "value": "Grass" } }
  }
}
```

### Why Explicit Types?

Roblox has many property types that look similar but serialize differently:

```json
// Color3 uses 0-1 floats
"Color": { "type": "Color3", "value": { "r": 0.5, "g": 0.5, "b": 0.5 } }

// Color3uint8 uses 0-255 integers
"Color": { "type": "Color3uint8", "value": { "r": 128, "g": 128, "b": 128 } }
```

Without explicit types, round-trip conversion would lose precision or fail silently.

See [.rbxjson Format](/file-formats/rbxjson) and [Property Types](/file-formats/property-types) for complete reference.

## Directory Structure

Extracted games follow a predictable directory structure:

```
MyGame/
├── rbxsync.json              # Project configuration
├── src/                      # Instance tree (required)
│   ├── Workspace/
│   │   ├── _meta.rbxjson     # Service properties
│   │   ├── Baseplate.rbxjson
│   │   └── SpawnLocation.rbxjson
│   ├── ServerScriptService/
│   │   └── Main.server.luau
│   ├── ReplicatedStorage/
│   │   └── Modules/
│   │       ├── _meta.rbxjson # Folder properties
│   │       └── Utils.luau
│   ├── StarterGui/
│   ├── StarterPack/
│   ├── StarterPlayer/
│   ├── Lighting.rbxjson      # Service as single file
│   └── Terrain/
│       └── terrain.rbxjson   # Terrain data
├── .rbxsync-backup/          # Auto-backup (for undo)
└── sourcemap.json            # For Luau LSP
```

### Service Directories

Top-level directories under `src/` correspond to Roblox services:

| Directory | Service |
|-----------|---------|
| `Workspace/` | game.Workspace |
| `ServerScriptService/` | game.ServerScriptService |
| `ReplicatedStorage/` | game.ReplicatedStorage |
| `ReplicatedFirst/` | game.ReplicatedFirst |
| `StarterGui/` | game.StarterGui |
| `StarterPack/` | game.StarterPack |
| `StarterPlayer/` | game.StarterPlayer |
| `ServerStorage/` | game.ServerStorage |
| `Lighting/` | game.Lighting |
| `SoundService/` | game.SoundService |

### Meta Files

Directories can have a `_meta.rbxjson` file to set properties on the folder instance:

```
ReplicatedStorage/
├── _meta.rbxjson          # Properties for the Folder itself
├── Module1.luau
└── Subfolder/
    ├── _meta.rbxjson      # Properties for Subfolder
    └── Module2.luau
```

This pattern allows folders to have attributes, tags, or custom properties.

### Name Disambiguation

When sibling instances share the same name, RbxSync adds suffixes:

```
Folder/
├── Button.rbxjson       # First instance
├── Button~2~.rbxjson    # Second instance named "Button"
└── Button~3~.rbxjson    # Third instance named "Button"
```

The `~N~` suffix is only for filesystem storage; the actual instance name remains "Button".

## Instance References

Instances can reference other instances (e.g., `ObjectValue.Value`, `Weld.Part0`).

### Reference IDs

Each instance receives a unique reference ID during extraction using Roblox's `GetDebugId()`:

```json
{
  "className": "Part",
  "referenceId": "ABC123DEF456",
  "properties": { ... }
}
```

### Storing References

References to other instances use the `Ref` type:

```json
{
  "className": "ObjectValue",
  "properties": {
    "Value": {
      "type": "Ref",
      "value": "ABC123DEF456"
    }
  }
}
```

### Null References

Unset references use `null`:

```json
{
  "className": "ObjectValue",
  "properties": {
    "Value": {
      "type": "Ref",
      "value": null
    }
  }
}
```

### Reference Resolution

During sync to Studio, RbxSync:

1. Builds a map of reference IDs to instances
2. Resolves `Ref` properties after all instances are created
3. Handles missing references gracefully (sets to nil)

## Property Handling

### Supported Types

RbxSync supports 40+ property types. Common categories:

| Category | Types |
|----------|-------|
| Primitives | `bool`, `int`, `int64`, `float`, `double`, `string` |
| Vectors | `Vector2`, `Vector2int16`, `Vector3`, `Vector3int16` |
| Colors | `Color3`, `Color3uint8`, `BrickColor` |
| Transforms | `CFrame` |
| UI | `UDim`, `UDim2`, `Rect` |
| Sequences | `NumberSequence`, `ColorSequence` |
| Ranges | `NumberRange` |
| Enums | `Enum` |
| Assets | `Content` |
| Special | `Font`, `Faces`, `Axes`, `PhysicalProperties` |

See [Property Types](/file-formats/property-types) for complete examples.

### Attributes

Instance attributes are stored in an `attributes` section:

```json
{
  "className": "Part",
  "properties": { ... },
  "attributes": {
    "health": { "type": "number", "value": 100 },
    "team": { "type": "string", "value": "red" },
    "spawnPoint": { "type": "Vector3", "value": { "x": 0, "y": 5, "z": 0 } }
  }
}
```

### Tags

CollectionService tags are stored as an array:

```json
{
  "className": "Part",
  "properties": { ... },
  "tags": ["interactable", "checkpoint", "glowing"]
}
```

## Binary Files (.rbxm)

RbxSync uses `.rbxm` files only for plugin distribution:

```bash
# Build the Studio plugin
rojo build plugin/default.project.json -o build/RbxSync.rbxm
```

The plugin binary is installed to Studio's plugins folder. Game data is never stored as `.rbxm` because binary files cannot be diffed or merged.

## Special Cases

### Terrain

Terrain data is stored in `src/Workspace/Terrain/terrain.rbxjson`:

```json
{
  "className": "Terrain",
  "properties": {
    "WaterColor": { "type": "Color3", "value": { "r": 0.13, "g": 0.56, "b": 0.78 } },
    "WaterReflectance": { "type": "float", "value": 1 },
    "WaterTransparency": { "type": "float", "value": 0.3 },
    "WaterWaveSize": { "type": "float", "value": 0.15 },
    "WaterWaveSpeed": { "type": "float", "value": 10 }
  },
  "terrainData": "base64-encoded-terrain-voxels..."
}
```

Terrain voxel data is base64-encoded to preserve binary fidelity.

### CSG Operations

UnionOperation, NegateOperation, and IntersectOperation instances store their serialized geometry data in the `.rbxjson` file. CSG operations are preserved through the AssetId property.

### Packages

Package links are preserved during extraction. The `PackageLink` property maintains the connection to the asset library.

## Complete Example

Here's a complete extracted game structure:

```
MyGame/
├── rbxsync.json
├── src/
│   ├── Workspace/
│   │   ├── _meta.rbxjson
│   │   ├── Baseplate.rbxjson
│   │   ├── SpawnLocation.rbxjson
│   │   └── Models/
│   │       ├── _meta.rbxjson
│   │       ├── Tree.rbxjson
│   │       └── Rock.rbxjson
│   ├── ServerScriptService/
│   │   ├── Main.server.luau
│   │   └── Systems/
│   │       ├── _meta.rbxjson
│   │       ├── Combat.server.luau
│   │       └── Inventory.server.luau
│   ├── ReplicatedStorage/
│   │   ├── Modules/
│   │   │   ├── _meta.rbxjson
│   │   │   ├── Utils.luau
│   │   │   └── Config.luau
│   │   └── Events/
│   │       ├── _meta.rbxjson
│   │       ├── PlayerDied.rbxjson
│   │       └── ItemPurchased.rbxjson
│   ├── StarterGui/
│   │   └── MainMenu/
│   │       ├── _meta.rbxjson
│   │       ├── Frame.rbxjson
│   │       └── PlayButton.rbxjson
│   ├── StarterPlayer/
│   │   └── StarterPlayerScripts/
│   │       └── Client.client.luau
│   └── Lighting.rbxjson
└── sourcemap.json
```

## Summary

| Decision | Rationale |
|----------|-----------|
| JSON for instances | Diff-friendly, editable, mergeable |
| Separate `.luau` files | LSP support, clean diffs |
| Explicit types | Round-trip fidelity |
| Directory = hierarchy | Predictable structure |
| `_meta.rbxjson` | Folder properties without ambiguity |
| Reference IDs | Cross-instance linking |
| Binary only for plugin | Game data must be diffable |

For detailed property specifications, see:
- [.luau Scripts](/file-formats/luau)
- [.rbxjson Format](/file-formats/rbxjson)
- [Property Types](/file-formats/property-types)
