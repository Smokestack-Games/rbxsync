# RbxSync E2E Testing Harness — Agent Guide

> Reference for AI agents using MCP tools to develop and test Roblox games with RbxSync.

---

## 1. Development Loop

The edit→sync→test→verify→iterate cycle:

```
Edit code (local .luau files)
  → sync_to_studio (push changes to Studio)
    → run_test (start playtest, capture console)
      → bot_observe / verify (check game state)
        → Iterate (fix issues, repeat)
```

**Critical rules:**

- **Stop playtest before editing code.** Changes made during a playtest do not affect the running session. Call `stop_playtest` first.
- **Always sync before testing.** `sync_to_studio` must be called after any file edit — Studio does not auto-reload changed files.
- **Scripts run only during playtest.** Scripts in ServerScriptService, LocalScript in StarterPlayerScripts, etc. are dormant in edit mode.
- **`run_code` is edit mode only.** It executes in plugin context, not inside the running game. Use `bot_query_server` (via bot tools) for runtime server state.
- **Changes to StarterPlayerScripts only affect new spawns.** If you change a LocalScript in StarterPlayerScripts, existing players won't see the update until they respawn.

---

## 2. Available MCP Tools

### Core Workflow

| Tool | Description |
|------|-------------|
| `sync_to_studio` | Push local file changes to Studio. Pass `project_dir`. Use `delete: true` to remove orphaned instances. |
| `extract_game` | Pull current Studio state to local files. Use before making changes to ensure you have the latest. |
| `run_test` | Start a playtest and capture all console output (prints, warnings, errors). Returns grouped output. |
| `stop_playtest` | End the active playtest. **Always call before editing code.** |
| `test_status` | Check if a playtest is currently running. |
| `run_code` | Execute Luau in plugin context (edit mode). Can create/modify instances, set properties. Cannot read runtime game state. |

### Verification

| Tool | Description |
|------|-------------|
| `verify` | Assert game state during a playtest. Checks properties, attributes, counts, distances, backpack items, leaderstats. Supports polling via `timeout` for async conditions. Returns `PASS`/`FAIL` with actual vs expected. |

### Bot Interaction (playtest only)

| Tool | Description |
|------|-------------|
| `bot_observe` | Get player state (position, health, inventory, nearby objects). Types: `state`, `nearby`, `npcs`, `inventory`, `find`. |
| `bot_move` | Navigate to a position `{x, y, z}` or object by name using PathfindingService. |
| `bot_action` | Perform actions: `equip`, `unequip`, `activate`, `deactivate`, `interact`, `jump`. |

### Instance Manipulation

| Tool | Description |
|------|-------------|
| `set_property` | Set a property on an instance by path. Works in edit mode. |
| `create_instance` | Create a new instance in the DataModel. |
| `delete_instance` | Delete an instance by path. |
| `read_properties` | Read all properties of an instance. |
| `explore_hierarchy` | List children of an instance. |
| `find_instances` | Find instances by name, class, or tag across the DataModel. |

### Multi-Place

| Tool | Description |
|------|-------------|
| `set_active_place` | Set the active Studio place by Place ID when working with multi-place projects. |

---

## 3. Verify Check Types

The `verify` tool accepts a `check` field that determines the assertion type. All checks return `PASS`/`FAIL` with `actual` and `expected` values on failure.

**Paths use `/` separators** (e.g., `Workspace/Map/Door`).

**Operators:** `eq`, `neq`, `gt`, `lt`, `gte`, `lte`, `contains`, `exists`, `not_exists`

### property — Check an instance property

```json
{
  "check": "property",
  "path": "Workspace/Door",
  "property": "Position",
  "operator": "eq",
  "expected": { "X": 10, "Y": 1, "Z": 0 }
}
```

CFrame and Vector3 are returned as `{X, Y, Z}`. Color3 as `{R, G, B}` (0–255). Enums as their name string.

### attribute — Check an instance attribute

```json
{
  "check": "attribute",
  "path": "Workspace/Chest",
  "attribute": "isOpen",
  "operator": "eq",
  "expected": true
}
```

### count — Count instances by tag or class+parent

By CollectionService tag:
```json
{
  "check": "count",
  "tag": "Enemy",
  "operator": "eq",
  "expected": 5
}
```

By class under a parent:
```json
{
  "check": "count",
  "class": "Part",
  "parent": "Workspace/Coins",
  "operator": "gt",
  "expected": 0
}
```

### find — Check if an instance exists

```json
{
  "check": "find",
  "path": "Workspace/SpawnedItem",
  "operator": "exists"
}
```

### children — List children with optional class filter

```json
{
  "check": "children",
  "path": "ServerScriptService",
  "class": "Script"
}
```

Returns the list of matching children. Use `operator: "contains"` with `expected: "MyScript"` to assert a specific child exists.

### distance — Distance between two instances

```json
{
  "check": "distance",
  "path": "Workspace/Player/HumanoidRootPart",
  "target": "Workspace/Checkpoint",
  "operator": "lt",
  "expected": 10
}
```

### backpack — Player has a tool in backpack

```json
{
  "check": "backpack",
  "item": "Sword",
  "operator": "exists"
}
```

Checks the first player's Backpack. Use `operator: "not_exists"` to assert an item is absent.

### leaderstat — Player stat value

```json
{
  "check": "leaderstat",
  "stat": "Coins",
  "operator": "gte",
  "expected": 50
}
```

Reads from the first player's `leaderstats` folder.

### timeout — Poll until a check passes

Any check can include `timeout` to retry until true (or fail after N seconds):

```json
{
  "check": "find",
  "path": "Workspace/SpawnedItem",
  "operator": "exists",
  "timeout": 10
}
```

Without `timeout`, all checks are instantaneous. With `timeout`, the Verifier polls until the check passes or the timeout expires.

---

## 4. Common Roblox Patterns

### Services

| Service | Purpose |
|---------|---------|
| `Workspace` | 3D world, Parts, Models, terrain |
| `ServerScriptService` | Scripts that run on the server |
| `ServerStorage` | Server-only storage (not replicated) |
| `ReplicatedStorage` | Shared between server and client |
| `StarterGui` | UI templates (copied to player on spawn) |
| `StarterPack` | Tools given to player on spawn |
| `StarterPlayer` | StarterCharacterScripts, StarterPlayerScripts |
| `Players` | Active player objects |
| `Lighting` | Lighting and atmosphere |
| `SoundService` | Ambient sounds |
| `Teams` | Team definitions |
| `CollectionService` | Tag-based instance grouping |

### Script Types

| Type | Runs On | Where to Place |
|------|---------|---------------|
| `Script` | Server | ServerScriptService, ServerStorage |
| `LocalScript` | Client | StarterGui, StarterPack, StarterPlayerScripts, StarterCharacterScripts |
| `ModuleScript` | Caller's context (server or client) | ReplicatedStorage (shared), ServerScriptService (server-only) |

Scripts in Workspace run on the **server** if they are a `Script`, **client** if `LocalScript`.

### Common Properties

| Property | Type | Description |
|----------|------|-------------|
| `Position` | Vector3 | World position |
| `CFrame` | CFrame | Position + orientation |
| `Size` | Vector3 | Dimensions |
| `Transparency` | number (0–1) | 0 = opaque, 1 = invisible |
| `CanCollide` | bool | Physics collision |
| `Anchored` | bool | Frozen in place (no physics) |
| `Name` | string | Instance name |
| `Parent` | Instance | Parent in hierarchy |
| `Enabled` | bool | Script / GUI enabled state |
| `Value` | any | Value object (IntValue, StringValue, etc.) |
| `Humanoid.Health` | number | Character health |
| `Humanoid.MaxHealth` | number | Max health |

---

## 5. Gotchas

- **Scripts only run during playtest.** In edit mode, no game logic executes. `run_code` can create instances but cannot trigger game events or call runtime APIs like `HttpService:GetAsync`.

- **`run_code` is plugin context, not game context.** It can read/write the DataModel structure but cannot access `game.Players:GetPlayers()` during a playtest as if it were a server script. Use `bot_query_server` for runtime server state.

- **Bot tools only work during an active playtest.** Calling `bot_observe`, `bot_move`, or `bot_action` outside a playtest will fail. Check `test_status` or call `run_test` first.

- **Stop playtest before editing code.** Code changes synced during an active playtest will not take effect. The session must be restarted.

- **Always sync before testing.** File edits are not automatically reflected in Studio. `sync_to_studio` must be called after each code change.

- **StarterPlayerScripts changes require respawn.** LocalScripts in `StarterPlayerScripts` are cloned to each player on spawn. Syncing changes won't affect the current session's existing players — they need to respawn.

- **HTTP requests must be enabled for bot tools.** `bot_observe`, `bot_move`, `bot_action` communicate with the running game via HTTP. Enable in Studio: Game Settings → Security → Allow HTTP Requests.

- **Instance paths use `/` not `.`** in `verify`, `read_properties`, `set_property`, and related tools. Example: `Workspace/Map/Door`, not `Workspace.Map.Door`.

- **`verify` runs in plugin context against the DataModel.** It reads the current state of instances, not a snapshot. For runtime state (player stats mid-game), combine `verify` (for structure) with `bot_query_server` (for runtime values) or use the `leaderstat` / `backpack` check types.
