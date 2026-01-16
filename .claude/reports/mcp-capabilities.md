# MCP Tool Capabilities Deep Dive

**Date:** 2026-01-16
**Purpose:** Document RbxSync MCP tool capabilities, limitations, and improvement opportunities

---

## Overview

RbxSync MCP Server provides 12 tools for AI-assisted Roblox development:

| Category | Tools |
|----------|-------|
| **Sync** | `extract_game`, `sync_to_studio` |
| **Git** | `git_status`, `git_commit` |
| **Execution** | `run_code`, `run_test` |
| **Bot Control** | `bot_observe`, `bot_move`, `bot_action`, `bot_command`, `bot_query_server`, `bot_wait_for` |

**Architecture:**
```
Claude/AI Agent ─► MCP Server (rbxsync-mcp) ─► HTTP Server (rbxsync-server) ─► Studio Plugin
                        ↓                              ↓
                   JSON-RPC/stdio                  HTTP :44755
```

---

## 1. Tool Capabilities

### extract_game

**Purpose:** Extract entire Roblox game from Studio to local filesystem

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `project_dir` | string | Yes | Directory to write files |
| `services` | string[] | No | Filter to specific services |
| `include_terrain` | bool | No | Include voxel terrain data (default: true) |

**Capabilities:**
- Extracts all services: Workspace, ServerScriptService, ReplicatedStorage, etc.
- Scripts → `.luau` files with type suffixes (`.server.luau`, `.client.luau`)
- Instances → `.rbxjson` with full property serialization (70+ types)
- Terrain → base64-encoded voxel data
- Handles name collisions with `~N~` suffixes
- Generates `sourcemap.json` for Luau LSP
- Incremental - clears src folder before extraction
- Instance references preserved via `GetDebugId()`

**Edge Cases:**
- Large games (1000+ instances): Works but slow, handled via chunked transfer
- Unions (CSG): Stored via AssetId, reconstructable
- Packages: PackageLink preserved
- Binary properties: Base64 encoded

**Returns:**
```
Successfully extracted game to /path/to/project. 247 files written.
```

---

### sync_to_studio

**Purpose:** Push local file changes back to Roblox Studio

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `project_dir` | string | Yes | Directory containing files |
| `delete` | bool | No | Delete orphaned instances in Studio |

**Capabilities:**
- Incremental sync - only sends modified files since last sync
- Handles all property types supported by `.rbxjson`
- Creates new instances from files
- Updates existing instances
- Optionally deletes orphans (instances in Studio but not on disk)
- Respects plugin sync direction toggle

**Edge Cases:**
- Sync skipped if extraction in progress
- Sync skipped if "Files → Studio" disabled in plugin
- Empty sync when no changes detected
- Reference resolution happens after all instances created

**Known Limitations:**
- Cannot recreate non-separable CSG operations from scratch
- Instance rename detection relies on reference IDs
- Large syncs chunked but may timeout on very large operations

**Returns:**
```
Successfully synced 15 instances to Studio (incremental sync, 3 of 50 files modified).
```

---

### run_code

**Purpose:** Execute arbitrary Luau code in Roblox Studio

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `code` | string | Yes | Luau code to execute |

**Capabilities:**
- Executes via `loadstring` in plugin context
- Has plugin-level API access
- Can query/modify game state
- Returns printed output

**Edge Cases:**
- Code with syntax errors returns error message
- Long-running code may timeout (30s default)
- Output truncated if very long

**Known Limitations:**
- Plugin context, not game context (cannot access LocalPlayer, etc.)
- Cannot simulate user input (VirtualInputManager unavailable)
- Cannot access game server during playtest (use `bot_query_server` instead)

**Returns:** Raw output from `print()` statements or error message

---

### run_test

**Purpose:** Run automated playtest and capture console output

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `duration` | int | No | Test duration in seconds (default: 5) |
| `mode` | string | No | "Play" (client) or "Run" (server only) |

**Capabilities:**
- Starts Studio playtest programmatically
- Captures all console output (prints, warnings, errors)
- Timestamps each message
- Categorizes by message type
- Runs built-in movement verification test
- Auto-stops after duration

**Edge Cases:**
- Test continues running even if no output
- Max wait is duration + 5 seconds
- Handles Studio already in playtest state

**Known Limitations:**
- Cannot start playtest if Studio plugin disconnected
- Duration is approximate (polling-based)
- Cannot simulate input events during test

**Returns:**
```
Test completed in 32.1s
Total messages: 142

=== ERRORS (1) ===
[0.80s] Error message here

=== WARNINGS (3) ===
[0.22s] Warning message here

=== OUTPUT (138) ===
[0.20s] [GameService] Initialized
```

---

### bot_observe

**Purpose:** Get game state during active playtest

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `type` | string | No | Observation type (default: "state") |
| `radius` | float | No | Search radius in studs |
| `query` | string | No | Search query for "find" type |

**Observation Types:**
- `state` - Full state: position, health, inventory, equipped, UI
- `nearby` - Objects within radius
- `npcs` - Characters/humanoids within radius
- `inventory` - Tools in backpack
- `find` - Search for objects by name

**Capabilities:**
- Returns structured JSON with game state
- Character position as Vector3
- Health/MaxHealth from Humanoid
- Tool names in inventory
- Currently equipped tool
- Visible UI elements
- Movement state

**Edge Cases:**
- No character: Returns error "No character found"
- No playtest: Returns error about playtest required

**Known Limitations (CRITICAL):**
- **Disabled in Creator Store plugin version**
- Full functionality requires custom plugin build
- 30 second timeout
- State observation should be throttled (~10/sec max)

---

### bot_move

**Purpose:** Move character to position/object using pathfinding

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `position` | {x,y,z} | No | Target coordinates |
| `objectName` | string | No | Name of object to navigate to |

Use `position` OR `objectName`, not both.

**Capabilities:**
- Uses PathfindingService for navigation
- Avoids obstacles automatically
- Reports reached/final position
- Handles stuck detection

**Edge Cases:**
- Path unreachable: Returns with `reached: false`
- Object not found: Returns error
- Long paths: May need multiple waypoints

**Known Limitations:**
- **Disabled in Creator Store plugin version**
- Max efficient path: ~500 studs before recompute
- Cannot climb TrussParts by default
- 60 second timeout for movement
- Humanoid can get stuck on complex geometry

---

### bot_action

**Purpose:** Perform character actions

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `action` | string | Yes | Action type |
| `name` | string | No | Tool/object name |

**Supported Actions:**
- `equip` - Equip tool from backpack
- `unequip` - Unequip current tool
- `activate` - Activate equipped tool (simulate click)
- `deactivate` - Stop activation
- `interact` - Interact with nearby object
- `jump` - Make character jump

**Capabilities:**
- Programmatic tool equipping/activation
- Jump via Humanoid.Jump = true
- Interaction with ProximityPrompts

**Known Limitations:**
- **Disabled in Creator Store plugin version**
- Cannot simulate keyboard/mouse input
- Interaction depends on game implementation
- 30 second timeout

---

### bot_command

**Purpose:** Generic bot command for advanced control

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `type` | string | Yes | Command category |
| `command` | string | Yes | Specific command |
| `args` | object | No | Command arguments |

**Command Types:**
- `move` - Movement commands
- `action` - Action commands
- `ui` - UI interaction commands
- `observe` - Observation commands

**UI Commands:**
- `clickButton` - Click GUI button
- `readText` - Read TextLabel content
- `fillTextBox` - Enter text in TextBox
- `getVisibleUI` - List visible UI elements

**Known Limitations:**
- **Disabled in Creator Store plugin version**
- UI paths must be exact
- UI must be visible to interact
- 60 second timeout

---

### bot_query_server

**Purpose:** Execute Luau on game server during playtest

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `code` | string | Yes | Luau code to execute |

**Capabilities:**
- Runs in server context during playtest
- Can query DataStores, leaderstats, services
- Returns expression results
- Useful for verifying server-side state

**Examples:**
```lua
-- Get player coins
game.Players:GetPlayers()[1].leaderstats.Coins.Value

-- Count players
#game.Players:GetPlayers()
```

**Known Limitations:**
- **Disabled in Creator Store plugin version**
- Requires active playtest
- Cannot modify game state (read-only recommended)

---

### bot_wait_for

**Purpose:** Wait for condition to become true

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `condition` | string | Yes | Luau boolean expression |
| `timeout` | float | No | Max wait seconds (default: 30) |
| `poll_interval` | int | No | Poll interval ms (default: 100) |
| `context` | string | No | "server" or "client" |

**Capabilities:**
- Polls condition at regular intervals
- Returns when condition true or timeout
- Reports elapsed time
- Server or client context

**Example Conditions:**
```lua
workspace:FindFirstChild('Ball') == nil
player.Inventory:FindFirstChild('Sword')
```

**Known Limitations:**
- **Disabled in Creator Store plugin version**
- Polling-based, not event-driven
- Timeout may be exceeded slightly due to poll interval

---

### git_status

**Purpose:** Get git repository status

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `project_dir` | string | Yes | Project directory |

**Capabilities:**
- Branch name
- Staged files
- Modified files
- Untracked files

**Returns:**
```
Branch: main
Staged (2):
  + src/ServerScriptService/Main.server.luau
Modified (1):
  ~ src/ReplicatedStorage/Config.luau
Untracked (3):
  ? src/Workspace/NewPart.rbxjson
```

---

### git_commit

**Purpose:** Commit changes to git

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `project_dir` | string | Yes | Project directory |
| `message` | string | Yes | Commit message |
| `files` | string[] | No | Specific files to commit |

**Capabilities:**
- Stage and commit in one operation
- Optional file filtering
- Returns commit hash

---

## 2. Known Limitations Summary

### Architecture Limitations

| Limitation | Affected Tools | Workaround |
|------------|----------------|------------|
| Plugin context only | `run_code` | Use `bot_query_server` for game context |
| Creator Store restrictions | All bot tools | Use custom plugin build |
| No input simulation | Bot tools | Use programmatic APIs |
| Polling-based | `bot_wait_for`, `run_test` | Accept latency |

### Roblox API Constraints

1. **VirtualInputManager** - Not accessible from plugins (Roblox internal)
2. **Direct input simulation** - Cannot simulate keyboard/mouse
3. **LoadString scope** - Plugin context, not game context
4. **Property access** - Some properties read-only in plugin API

### Performance Constraints

| Constraint | Limit | Impact |
|------------|-------|--------|
| Observation throttle | ~10/sec | Excessive calls may be rate-limited |
| Pathfinding range | ~500 studs | Long paths need recomputation |
| HTTP timeout | 30-60s | Long operations may fail |
| Extraction chunking | Variable | Large games are slow |

---

## 3. Edge Cases That Fail

### Critical Failures

1. **Union deletion during extract (RBXSYNC-38)**
   - Unions may be deleted when extraction fails mid-process
   - Status: Active bug, P1 priority

2. **Non-separable CSG**
   - Unions created via AssetId that cannot be separated
   - Cannot recreate geometry from scratch

3. **Instance renames (RBXSYNC-5)**
   - Renaming instance in Studio doesn't propagate correctly
   - Reference ID preserved but name mismatch

### Recoverable Failures

| Edge Case | Behavior | Recovery |
|-----------|----------|----------|
| Plugin disconnected | Tools return connection error | Reconnect plugin |
| No playtest active | Bot tools fail | Call `run_test` first |
| Path unreachable | `bot_move` returns `reached: false` | Try different path |
| UI not visible | UI commands fail | Wait/verify UI state |
| Syntax error in code | Returns error message | Fix code |

---

## 4. Missing Tools for AI Workflows

### High Priority

| Tool | Purpose | Use Case |
|------|---------|----------|
| `screenshot` | Capture Studio viewport | Visual debugging, UI testing |
| `diff_preview` | Show pending sync changes | Verification before sync |
| `undo_sync` | Revert last sync operation | Error recovery |
| `validate_code` | Syntax check without execution | Pre-sync validation |

### Medium Priority

| Tool | Purpose | Use Case |
|------|---------|----------|
| `get_selection` | Get currently selected instances | Context-aware editing |
| `set_selection` | Select instances in Studio | UI navigation |
| `get_properties` | Read instance properties directly | Quick inspection |
| `set_properties` | Set properties without file roundtrip | Quick fixes |
| `search_instances` | Find instances by criteria | Discovery |
| `get_descendants` | Get instance tree subset | Scoped exploration |

### Low Priority

| Tool | Purpose | Use Case |
|------|---------|----------|
| `get_logs` | Historical console output | Debugging |
| `set_breakpoint` | Add debugger breakpoints | Advanced debugging |
| `profile_performance` | Get performance metrics | Optimization |
| `list_assets` | Inventory of game assets | Asset management |

---

## 5. Comparison: What AI Tools Need

### Claude Code (Current)

**Has:**
- Full MCP tool access
- File reading/writing
- Command execution
- Multi-step reasoning

**Needs from RbxSync:**
- ✅ Extract/sync (provided)
- ✅ Code execution (provided)
- ✅ Test running (provided)
- ⚠️ Bot control (limited in Creator Store)
- ❌ Screenshots
- ❌ Selection control

### Antigravity (Hypothetical AI Roblox Tool)

Based on competitive analysis, an ideal AI tool would need:

| Capability | RbxSync Status | Gap |
|------------|----------------|-----|
| Real-time sync | ✅ Full | - |
| Code execution | ✅ Full | - |
| Console streaming | ✅ Full | - |
| Automated testing | ✅ Full | - |
| Bot control | ⚠️ Limited | Creator Store restrictions |
| Visual feedback | ❌ None | Screenshots needed |
| Asset browsing | ❌ None | Model search exists (`insert_model`) |
| Voice commands | ❌ None | Out of scope |
| Natural language queries | ❌ None | LLM layer responsibility |

### Feature Gap Analysis

**What RbxSync does better than competitors:**
1. Native MCP integration (unique)
2. Two-way sync (better than Rojo)
3. E2E test automation (unique)
4. Console streaming (unique)
5. Bot control framework (unique)

**What's missing for complete AI workflow:**
1. Visual feedback (screenshots)
2. Full bot control in public release
3. Property inspection without file reads
4. Undo/recovery mechanisms
5. Asset library integration

---

## 6. Recommendations

### Short-term (v1.2)

1. **Document bot control limitations** - Clarify Creator Store restrictions
2. **Add `validate_code` tool** - Syntax check before sync
3. **Add `diff_preview` tool** - Show pending changes

### Medium-term (v1.3)

1. **Implement screenshot tool** - Capture viewport for AI vision
2. **Add property inspection** - Direct property access without files
3. **Enable full bot control** - Resolve Creator Store restrictions or provide alternative distribution

### Long-term

1. **Selection/manipulation tools** - Direct instance control
2. **Asset library integration** - Better model/asset discovery
3. **Performance profiling** - Built-in optimization tools
4. **Multi-agent coordination** - Concurrent AI agent support

---

## Appendix: HTTP Endpoints

The MCP server communicates with rbxsync-server via these endpoints:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | Check connection |
| `/extract/start` | POST | Begin extraction |
| `/extract/status` | GET | Poll extraction progress |
| `/extract/finalize` | POST | Complete extraction |
| `/sync/incremental` | POST | Get changed files |
| `/sync/batch` | POST | Apply sync operations |
| `/diff` | POST | Get Studio vs disk diff |
| `/run` | POST | Execute Luau code |
| `/sync/command` | POST | Test runner commands |
| `/bot/*` | Various | Bot control endpoints |
| `/git/status` | POST | Git status |
| `/git/commit` | POST | Git commit |

---

*Generated by Claude Code analysis of rbxsync-mcp v1.1.2*
