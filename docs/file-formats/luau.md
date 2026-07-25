# Scripts on Disk

Scripts are stored as plain Luau files with naming conventions that determine their script type. A script's `.luau` file is the editable source of truth for its code.

## Naming Conventions

| Extension | Script Type | Runs On |
|-----------|-------------|---------|
| `.server.luau` | Script | Server |
| `.client.luau` | LocalScript | Client |
| `.luau` | ModuleScript | Imported |

## Examples

### Server Script
`src/ServerScriptService/Main.server.luau`

```lua
local Players = game:GetService("Players")

Players.PlayerAdded:Connect(function(player)
    print("Welcome", player.Name)
end)
```

### Client Script
`src/StarterPlayer/StarterPlayerScripts/Client.client.luau`

```lua
local Players = game:GetService("Players")
local player = Players.LocalPlayer

print("Client loaded for", player.Name)
```

### Module Script
`src/ReplicatedStorage/Modules/Utils.luau`

```lua
local Utils = {}

function Utils.formatNumber(n)
    return string.format("%d", n)
end

return Utils
```

## Script Properties

A script's `.luau` file holds only its source. Everything else about the
instance — its class, name, and any non-default properties like `Enabled` or
`RunContext` — lives on the script's node in the [context file](/file-formats/rbxjson),
where its `Source` is replaced by a `sourcePath` pointing back at the `.luau`
file:

```json
{
  "className": "Script",
  "name": "Main",
  "properties": {
    "Enabled": { "type": "bool", "value": true }
  },
  "sourcePath": "src/ServerScriptService/Main.server.luau"
}
```

## Automatic Detection

When syncing:
- Files ending in `.server.luau` become `Script` instances
- Files ending in `.client.luau` become `LocalScript` instances
- Files ending in `.luau` (no prefix) become `ModuleScript` instances

The file name (without extension) becomes the instance name.
