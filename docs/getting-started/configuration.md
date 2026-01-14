# Project Configuration

The `rbxsync.json` file configures your project settings.

## Basic Configuration

```json
{
  "name": "MyGame",
  "tree": "./src",
  "assets": "./assets"
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `name` | Project folder name | Display name for the project |
| `tree` | `./src` | Path to the instance tree |
| `assets` | `./assets` | Path for binary assets (meshes, images, sounds) |

## Custom Directory Mapping

Use `treeMapping` to customize how DataModel paths map to filesystem paths. This is useful for:
- Matching existing Rojo project structures
- Using shorter directory names
- Organizing code by feature instead of service

```json
{
  "name": "MyGame",
  "tree": "./src",
  "treeMapping": {
    "ServerScriptService": "src/server",
    "ReplicatedStorage": "src/shared",
    "StarterPlayer/StarterPlayerScripts": "src/client",
    "Workspace/Maps": "src/maps"
  }
}
```

With this configuration:
- Scripts in `src/server/` sync to `ServerScriptService`
- Scripts in `src/shared/` sync to `ReplicatedStorage`
- Scripts in `src/client/` sync to `StarterPlayer.StarterPlayerScripts`

## Extraction Configuration

Control how games are extracted:

```json
{
  "config": {
    "extractBinaryAssets": true,
    "binaryAssetTypes": ["Mesh", "Image", "Sound", "Animation"],
    "excludeServices": ["CoreGui", "CorePackages"],
    "excludeClasses": [],
    "scriptSourceMode": "external",
    "terrainMode": "voxelData",
    "csgMode": "assetReference",
    "chunkSize": 1000
  }
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `extractBinaryAssets` | `true` | Extract meshes, images, sounds |
| `binaryAssetTypes` | All | Types to extract |
| `excludeServices` | CoreGui, etc. | Services to skip |
| `excludeClasses` | `[]` | Classes to skip |
| `scriptSourceMode` | `external` | `external` (files) or `inline` (in .rbxjson) |
| `terrainMode` | `voxelData` | `voxelData`, `propertiesOnly`, or `skip` |
| `csgMode` | `assetReference` | `assetReference`, `localMesh`, or `skip` |
| `chunkSize` | 1000 | Max instances per extraction batch |

## Sync Configuration

```json
{
  "sync": {
    "mode": "bidirectional",
    "conflictResolution": "prompt",
    "autoSync": false,
    "watchPaths": ["./src"]
  }
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `mode` | `bidirectional` | `push`, `pull`, or `bidirectional` |
| `conflictResolution` | `prompt` | `prompt`, `keepLocal`, `keepRemote`, `autoMerge` |
| `autoSync` | `false` | Auto-sync on file changes |
| `watchPaths` | `["./src"]` | Paths to watch for changes |

## Wally Package Support

RbxSync supports [Wally](https://wally.run/) packages. When enabled, packages are preserved during extraction and excluded from file watching to prevent accidental overwrites.

```json
{
  "packages": {
    "enabled": true,
    "sharedPackagesPath": "ReplicatedStorage/Packages",
    "serverPackagesPath": "ServerScriptService/Packages",
    "excludeFromWatch": true,
    "preserveOnExtract": true,
    "packagesFolder": "Packages"
  }
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Enable Wally package support |
| `sharedPackagesPath` | `ReplicatedStorage/Packages` | DataModel path for shared packages |
| `serverPackagesPath` | `ServerScriptService/Packages` | DataModel path for server packages |
| `excludeFromWatch` | `true` | Don't sync package file changes to Studio |
| `preserveOnExtract` | `true` | Keep local packages instead of overwriting from Studio |
| `packagesFolder` | `Packages` | Filesystem folder name for packages |

### How It Works

1. **File Watching**: Files in `Packages/` directories are ignored during live sync. This prevents your Wally packages from being accidentally synced back to Studio.

2. **Extraction**: When you extract a game, local Packages folders are preserved from your backup instead of being overwritten by Studio's version. This ensures your `wally.toml` dependencies stay intact.

3. **Wally Workflow**: Use Wally as normal to install packages:
   ```bash
   wally install
   ```
   Then sync your game code separately with RbxSync.

### Using with Rojo

You can use RbxSync alongside Rojo for Wally packages:
- Use Rojo to sync your `Packages/` folder
- Use RbxSync for everything else

Or use RbxSync exclusively by installing packages with Wally and enabling the `packages` config.

## Migrating from Rojo

If you have an existing Rojo project, migrate automatically:

```bash
rbxsync migrate
```

This reads your `default.project.json` and creates an equivalent `rbxsync.json` with matching directory mappings.

Example Rojo project:

```json
{
  "name": "MyGame",
  "tree": {
    "$className": "DataModel",
    "ServerScriptService": { "$path": "src/server" },
    "ReplicatedStorage": { "$path": "src/shared" }
  }
}
```

Converts to:

```json
{
  "name": "MyGame",
  "tree": "./src",
  "treeMapping": {
    "ServerScriptService": "src/server",
    "ReplicatedStorage": "src/shared"
  }
}
```

Your Rojo files are preserved—you can use both tools side-by-side during migration.

## Full Example

```json
{
  "name": "AwesomeGame",
  "tree": "./src",
  "assets": "./assets",
  "treeMapping": {
    "ServerScriptService": "src/server",
    "ReplicatedStorage": "src/shared",
    "StarterPlayer/StarterPlayerScripts": "src/client",
    "StarterGui": "src/ui"
  },
  "config": {
    "extractBinaryAssets": true,
    "scriptSourceMode": "external",
    "terrainMode": "voxelData"
  },
  "sync": {
    "mode": "bidirectional",
    "autoSync": false
  },
  "packages": {
    "enabled": true,
    "preserveOnExtract": true
  }
}
```
