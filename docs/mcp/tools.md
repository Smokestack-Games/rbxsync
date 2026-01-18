# MCP Tools Reference

Complete reference for all RbxSync MCP tools.

## Prerequisites

### Plugin Security

The `run_code` and `run_test` tools require `loadstring` to be available. This should work automatically for plugins with PluginSecurity level.

If you see "loadstring not available" errors, check:
1. The plugin is installed correctly in your Plugins folder
2. Studio output shows `[RbxSync] loadstring available - run:code enabled`

::: warning
If loadstring is not available, the `run_code` and `run_test` tools will not work. Other sync features will still function normally.
:::

---

## Core Sync Tools

### extract_game

Extract the connected game to git-friendly files on disk.

**Input:**
```json
{
  "project_dir": "/Users/you/MyGame",
  "services": ["Workspace", "ReplicatedStorage"],
  "include_terrain": true
}
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `project_dir` | string | Yes | - | Directory to extract files to |
| `services` | string[] | No | all | Specific services to extract |
| `include_terrain` | boolean | No | true | Include terrain voxel data |

**Output:**
```json
{
  "success": true,
  "message": "Successfully extracted game to /Users/you/MyGame. 1247 files written."
}
```

---

### sync_to_studio

Sync local file changes back to Roblox Studio. Uses incremental sync to only push modified files.

**Input:**
```json
{
  "project_dir": "/Users/you/MyGame",
  "delete": false
}
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `project_dir` | string | Yes | - | Directory containing project files to sync |
| `delete` | boolean | No | false | Delete orphaned instances in Studio that don't exist locally |

**Output:**
```json
{
  "success": true,
  "message": "Successfully synced 5 instances to Studio (incremental sync, 3 of 100 files modified)."
}
```

---

### run_code

Execute Luau code in Roblox Studio.

**Input:**
```json
{
  "code": "print('Hello from MCP!')"
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `code` | string | Yes | Luau code to execute in Studio |

**Output:**
```json
{
  "success": true,
  "output": "Hello from MCP!"
}
```

---

### run_test

Run an automated playtest in Studio and capture console output. Starts a play session, captures all prints/warnings/errors, then stops and returns output.

**Input:**
```json
{
  "duration": 10,
  "mode": "Play"
}
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `duration` | number | No | 5 | Test duration in seconds |
| `mode` | string | No | "Play" | "Play" for solo playtest (F5), "Run" for server simulation |

**Output:**
```json
{
  "success": true,
  "message": "Test completed in 10.0s\nTotal messages: 5\n\n=== ERRORS (1) ===\n[0.50s] Script error at line 10\n\n=== OUTPUT (4) ===\n[0.10s] Game started\n[0.20s] Player spawned"
}
```

---

## Introspection Tools

### read_properties

Read all properties of an instance at the given path. Returns className, name, and all serialized properties. Useful for inspecting instance state without running code.

**Input:**
```json
{
  "path": "Workspace/SpawnLocation"
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Instance path (e.g., "Workspace/SpawnLocation") |

**Output:**
```json
{
  "success": true,
  "data": {
    "className": "SpawnLocation",
    "name": "SpawnLocation",
    "path": "Workspace/SpawnLocation",
    "properties": {
      "Anchored": true,
      "Position": [0, 5, 0],
      "Size": [6, 1, 6],
      "AllowTeamChangeOnTouch": false,
      "Duration": 10,
      "Enabled": true,
      "Neutral": true,
      "TeamColor": "Medium stone grey"
    },
    "attributes": {},
    "tags": []
  }
}
```

---

### explore_hierarchy

Explore the game hierarchy to discover instances. Returns a tree of instances with their className, name, and childCount.

**Input:**
```json
{
  "path": "Workspace",
  "depth": 2
}
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `path` | string | No | - | Starting path (omit for top-level services) |
| `depth` | number | No | 1 | Depth limit (max: 10) |

**Output:**
```
Workspace [Workspace] (15 children)
  Camera [Camera]
  Terrain [Terrain]
  SpawnLocation [SpawnLocation]
  Baseplate [Part]
  Enemies [Folder] (3 children...)
```

---

### find_instances

Find instances matching search criteria. Searches by className, name pattern, and/or within a specific parent path.

**Input:**
```json
{
  "className": "Script",
  "name": "Main*",
  "parent": "ServerScriptService",
  "limit": 50
}
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `className` | string | No* | - | Filter by ClassName (e.g., "Part", "Script") |
| `name` | string | No* | - | Filter by Name (supports * wildcard) |
| `parent` | string | No* | - | Search within path (omit for entire game) |
| `limit` | number | No | 100 | Max results (max: 1000) |

*At least one filter (className, name, or parent) is required.

**Output:**
```
Found 3 instances:

  ServerScriptService/MainHandler [Script]
  ServerScriptService/Systems/MainLoop [Script]
  ServerScriptService/Core/MainManager [Script]
```

---

### insert_model

Insert a model from the Roblox marketplace into the game. Uses InsertService:LoadAsset to fetch the model by asset ID.

**Input:**
```json
{
  "assetId": 123456789,
  "parent": "Workspace/Models"
}
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `assetId` | number | Yes | - | Roblox asset ID of the model to insert |
| `parent` | string | No | "Workspace" | Parent path to insert into |

**Output:**
```json
{
  "success": true,
  "message": "Successfully inserted model:\n  Name: CoolSword\n  Path: Workspace/Models/CoolSword\n  ClassName: Tool"
}
```

---

## Bot Controller Tools

AI-powered automated gameplay testing tools. Must be called during an active playtest (after `run_test` or manual F5).

::: warning HTTP Required
All bot tools (`bot_observe`, `bot_move`, `bot_action`, `bot_wait_for`, `bot_command`, `bot_query_server`) require HTTP Requests to be enabled in your game settings.

**To enable:** Game Settings → Security → Allow HTTP Requests

Note: `run_test` does NOT require HTTP - it uses script injection and works without this setting.

See [HTTP Requirement for Bot Testing](/ai-testing#http-requirement-for-bot-testing) for details.
:::

### bot_observe

Observe current game state during a playtest. Returns character position, health, inventory, nearby objects/NPCs, and visible UI.

**Input:**
```json
{
  "type": "state",
  "radius": 50,
  "query": "Coin"
}
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `type` | string | No | "state" | Observation type: "state" (full), "nearby" (objects), "npcs", "inventory", "find" (search) |
| `radius` | number | No | 50 | Search radius in studs (for nearby/npcs) |
| `query` | string | No | - | Search query (for find type) |

**Output:**
```json
{
  "success": true,
  "data": {
    "position": { "x": 10, "y": 5, "z": 20 },
    "health": 100,
    "maxHealth": 100,
    "inventory": ["Sword", "Shield"],
    "equipped": "Sword",
    "nearby": [
      { "name": "Coin", "distance": 5.2, "className": "Part" },
      { "name": "NPC", "distance": 12.0, "className": "Model" }
    ]
  }
}
```

---

### bot_move

Move character to a position or named object using PathfindingService. The character will navigate around obstacles.

**Input:**
```json
{
  "position": { "x": 10, "y": 5, "z": 20 }
}
```

Or:

```json
{
  "objectName": "Checkpoint1"
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `position` | object | No* | Target position {x, y, z} |
| `objectName` | string | No* | Name of object to navigate to |

*Use position OR objectName (one is required).

**Output:**
```json
{
  "success": true,
  "message": "Successfully reached destination. Final position: {\"x\":10,\"y\":5,\"z\":20}"
}
```

---

### bot_action

Perform character actions: equip/unequip tools, activate abilities, interact with objects.

**Input:**
```json
{
  "action": "equip",
  "name": "Sword"
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `action` | string | Yes | Action: "equip", "unequip", "activate", "deactivate", "interact", "jump" |
| `name` | string | No | Tool or object name (required for equip, interact) |

**Output:**
```json
{
  "success": true,
  "message": "Action 'equip' completed: \"equipped\""
}
```

---

### bot_wait_for

Wait for a Luau condition to become true during an active playtest. Polls the condition at regular intervals until it returns true or timeout.

**Input:**
```json
{
  "condition": "workspace:FindFirstChild('Ball') == nil",
  "timeout": 30,
  "poll_interval": 100,
  "context": "server"
}
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `condition` | string | Yes | - | Luau condition code that returns true when met |
| `timeout` | number | No | 30 | Timeout in seconds |
| `poll_interval` | number | No | 100 | Poll interval in milliseconds |
| `context` | string | No | "server" | Execution context: "server" or "client" |

**Output:**
```json
{
  "success": true,
  "message": "Condition met after 2.50s"
}
```

---

### bot_command

Send a generic bot command for advanced character control. Supports movement, actions, UI interactions, and observations.

**Input:**
```json
{
  "type": "action",
  "command": "clickButton",
  "args": { "buttonName": "PlayButton" }
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `type` | string | Yes | Command type: "move", "action", "ui", "observe" |
| `command` | string | Yes | Command name (e.g., "moveTo", "equipTool", "clickButton") |
| `args` | object | No | Command arguments as JSON object |

**Output:**
```json
{
  "success": true,
  "message": "Command 'action.clickButton' result:\n{\"clicked\": true}"
}
```

---

### bot_query_server

Execute Luau code on the game server during an active playtest. Use this to query game state that only exists on the server (currency, DataStores, services).

**Input:**
```json
{
  "code": "game.Players:GetPlayers()[1].leaderstats.Coins.Value"
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `code` | string | Yes | Luau code to execute on server during playtest |

**Output:**
```json
{
  "success": true,
  "message": "Server query result (context: server):\n500"
}
```

---

## Harness Tools

Multi-session AI game development tracking. Use these tools to maintain context across development sessions.

### harness_init

Initialize harness for a project. Creates the `.rbxsync/harness` directory structure with `game.yaml` and `features.yaml`. Call this once at the start of a new game project.

**Input:**
```json
{
  "project_dir": "/Users/you/MyGame",
  "game_name": "Epic Adventure",
  "description": "An action RPG with procedural dungeons",
  "genre": "RPG"
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `project_dir` | string | Yes | Project directory path |
| `game_name` | string | Yes | Name of the game being developed |
| `description` | string | No | Game description |
| `genre` | string | No | Game genre (e.g., "Obby", "Tycoon", "Simulator") |

**Output:**
```json
{
  "success": true,
  "message": "Harness initialized at /Users/you/MyGame/.rbxsync/harness. Game ID: game_abc123"
}
```

---

### harness_status

Get current harness state for a project. Returns game info, features list with status summary, and recent sessions.

**Input:**
```json
{
  "project_dir": "/Users/you/MyGame"
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `project_dir` | string | Yes | Project directory path |

**Output:**
```
=== Harness Status ===

Game: Epic Adventure
Description: An action RPG with procedural dungeons

Features: 5 total (2 planned, 1 in progress, 2 completed, 0 blocked)

Feature List:
  - [feat_001] Player Combat (completed)
  - [feat_002] Inventory System (completed)
  - [feat_003] Dungeon Generator (in_progress)
  - [feat_004] Boss Battles (planned)
  - [feat_005] Multiplayer Support (planned)

Recent Sessions:
  - session_abc (ended, 2 features)
    Summary: Implemented basic combat and inventory
```

---

### harness_session_start

Start a new development session. Creates a session log to track work done across this conversation. Returns a session ID that can be used to end the session later.

**Input:**
```json
{
  "project_dir": "/Users/you/MyGame",
  "initial_goals": "Implement dungeon room generation and enemy spawning"
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `project_dir` | string | Yes | Project directory path |
| `initial_goals` | string | No | Initial goals/focus for this session |

**Output:**
```json
{
  "success": true,
  "message": "Session started. ID: session_xyz123\nPath: /Users/you/MyGame/.rbxsync/harness/sessions/session_xyz123.yaml"
}
```

---

### harness_session_end

End a development session. Updates the session log with summary and handoff notes for future sessions.

**Input:**
```json
{
  "project_dir": "/Users/you/MyGame",
  "session_id": "session_xyz123",
  "summary": "Implemented 3 room types and enemy spawner",
  "handoff_notes": [
    "Room connections need pathfinding validation",
    "Enemy AI is placeholder - needs proper state machine"
  ]
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `project_dir` | string | Yes | Project directory path |
| `session_id` | string | Yes | Session ID to end |
| `summary` | string | No | Summary of accomplishments |
| `handoff_notes` | string[] | No | Notes for future sessions |

**Output:**
```json
{
  "success": true,
  "message": "Session ended successfully."
}
```

---

### harness_feature_update

Create or update a feature in the project. Features track game functionality being developed across sessions. Provide `feature_id` to update an existing feature, or `name` to create a new one.

**Input:**
```json
{
  "project_dir": "/Users/you/MyGame",
  "name": "Player Combat",
  "description": "Melee and ranged combat system with combo chains",
  "status": "in_progress",
  "priority": "high",
  "tags": ["core", "gameplay"],
  "session_id": "session_xyz123"
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `project_dir` | string | Yes | Project directory path |
| `feature_id` | string | No | Feature ID for updates (omit for new features) |
| `name` | string | No* | Feature name (required for new features) |
| `description` | string | No | Feature description |
| `status` | string | No | Status: "planned", "in_progress", "completed", "blocked", "cancelled" |
| `priority` | string | No | Priority: "low", "medium", "high", "critical" |
| `tags` | string[] | No | Tags for categorization |
| `add_note` | string | No | Note to add to the feature |
| `session_id` | string | No | Session ID working on feature |

**Output:**
```json
{
  "success": true,
  "message": "Feature feat_001: created successfully"
}
```

---

## Git Tools

### git_status

Get the git status of a project.

**Input:**
```json
{
  "project_dir": "/Users/you/MyGame"
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `project_dir` | string | Yes | The project directory |

**Output:**
```
Branch: main

Staged (2):
  + src/ServerScriptService/Main.server.luau
  + src/ReplicatedStorage/Config.luau

Modified (1):
  ~ src/Workspace/Level.rbxm

Untracked (0):
```

---

### git_commit

Commit changes to git.

**Input:**
```json
{
  "project_dir": "/Users/you/MyGame",
  "message": "Add player spawning logic",
  "files": ["src/ServerScriptService/Main.server.luau"]
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `project_dir` | string | Yes | The project directory |
| `message` | string | Yes | The commit message |
| `files` | string[] | No | Specific files to commit (omit for all staged) |

**Output:**
```json
{
  "success": true,
  "message": "Committed: abc1234def5678"
}
```

---

## Example Workflow

Here's how an AI might use these tools:

1. **Extract current state**
   ```json
   { "tool": "extract_game", "arguments": { "project_dir": "/Users/you/MyGame" } }
   ```

2. **Explore the hierarchy**
   ```json
   { "tool": "explore_hierarchy", "arguments": { "path": "Workspace", "depth": 2 } }
   ```

3. **Modify code** (using file tools)

4. **Sync changes**
   ```json
   { "tool": "sync_to_studio", "arguments": { "project_dir": "/Users/you/MyGame" } }
   ```

5. **Run test**
   ```json
   { "tool": "run_test", "arguments": { "duration": 10, "mode": "Play" } }
   ```

6. **Observe game state**
   ```json
   { "tool": "bot_observe", "arguments": { "type": "state" } }
   ```

7. **Test gameplay**
   ```json
   { "tool": "bot_move", "arguments": { "objectName": "Checkpoint1" } }
   ```

8. **Check output for errors and fix**

9. **Commit changes**
   ```json
   { "tool": "git_commit", "arguments": { "project_dir": "/Users/you/MyGame", "message": "Fix player spawn bug" } }
   ```
