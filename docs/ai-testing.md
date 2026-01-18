# AI-Assisted E2E Testing Workflow

RbxSync enables AI agents (Claude, Cursor, etc.) to autonomously test Roblox games through MCP (Model Context Protocol). This guide explains how to set up and use AI-powered end-to-end testing.

## Overview

AI-assisted E2E testing allows AI agents to:

- **Write code** and sync changes to Studio instantly
- **Start playtests** and capture console output
- **Control characters** using pathfinding and actions
- **Observe game state** including health, inventory, and nearby objects
- **Query server data** like currency, DataStores, and services
- **Wait for conditions** to verify game behavior
- **Iterate autonomously** based on test results

This creates a development loop where an AI can implement features, test them in a real game environment, observe results, and fix issues without human intervention.

## Prerequisites

Before using AI-assisted testing:

1. **RbxSync server running**
   ```bash
   rbxsync serve
   ```

2. **Studio connected**
   - RbxSync plugin installed in Roblox Studio
   - Plugin connected (green indicator in widget)
   - A game open with at least one spawn point

3. **MCP configured**
   - AI client (Claude Code, Cursor, etc.) configured with RbxSync MCP server
   - See [MCP Setup](/mcp/setup) for configuration details

4. **HTTP Requests enabled** (for interactive bot testing)
   - Required for: `bot_observe`, `bot_move`, `bot_action`, `bot_query_server`, `bot_wait_for`, `bot_command`
   - NOT required for: `run_test` (uses script injection)
   - See [HTTP Requirement](#http-requirement-for-bot-testing) below

## The Testing Loop

AI-assisted testing follows a cycle:

```
┌─────────────────────────────────────────────────────────────┐
│  1. Write Code                                               │
│     └─ AI writes/modifies Luau scripts                       │
├─────────────────────────────────────────────────────────────┤
│  2. Sync to Studio                                           │
│     └─ sync_to_studio pushes changes                         │
├─────────────────────────────────────────────────────────────┤
│  3. Start Playtest                                           │
│     └─ run_test starts F5 session with output capture        │
├─────────────────────────────────────────────────────────────┤
│  4. Observe & Act                                            │
│     ├─ bot_observe → Get position, health, inventory         │
│     ├─ bot_move → Navigate to locations/objects              │
│     ├─ bot_action → Equip tools, interact, jump              │
│     └─ bot_query_server → Check server-side state            │
├─────────────────────────────────────────────────────────────┤
│  5. Verify Results                                           │
│     ├─ Check console output for errors                       │
│     ├─ bot_wait_for → Wait for conditions                    │
│     └─ Validate expected state changes                       │
├─────────────────────────────────────────────────────────────┤
│  6. Iterate                                                  │
│     └─ If issues found, return to step 1                     │
└─────────────────────────────────────────────────────────────┘
```

## MCP Tools Reference

### Core Workflow Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `sync_to_studio` | Push local file changes to Studio | After editing code |
| `run_test` | Start playtest with console capture | To test game behavior |
| `run_code` | Execute Luau in plugin context | Setup test fixtures, spawn items |

### Bot Control Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `bot_observe` | Get player state (position, health, inventory) | Check current game state |
| `bot_move` | Pathfind to location or object | Navigate to NPCs, items, areas |
| `bot_action` | Equip tools, interact, jump | Perform gameplay actions |
| `bot_query_server` | Execute Luau on game server | Check DataStores, currency, services |
| `bot_wait_for` | Wait for condition to be true | Verify async state changes |

### Detailed Tool Usage

#### run_test

Start an automated playtest and capture all console output.

```json
{
  "duration": 30,
  "mode": "Play"
}
```

**Parameters:**
- `duration` - Test length in seconds (default: 5)
- `mode` - "Play" for solo, "Run" for server simulation

**Returns:** Console output grouped by errors, warnings, and prints with timestamps.

#### bot_observe

Get current game state during playtest.

```json
{
  "type": "state"
}
```

**Types:**
- `state` - Full snapshot: position, health, inventory, nearby objects
- `nearby` - Objects within radius
- `npcs` - NPCs/humanoids within radius
- `inventory` - Tools in backpack
- `find` - Search for objects by name (use with `query` parameter)

**Example response:**
```json
{
  "position": {"x": 0, "y": 5, "z": 0},
  "health": 100,
  "maxHealth": 100,
  "inventory": ["Sword", "Health Potion"],
  "equipped": "Sword",
  "nearbyObjects": [
    {"name": "ShopNPC", "distance": 15.2, "className": "Model"}
  ]
}
```

#### bot_move

Navigate character using PathfindingService.

```json
{
  "objectName": "ShopNPC"
}
```

**Or by coordinates:**
```json
{
  "position": {"x": 50, "y": 5, "z": 30}
}
```

**Returns:** Whether destination was reached, final position, and path length.

#### bot_action

Perform character actions.

```json
{
  "action": "equip",
  "name": "Sword"
}
```

**Actions:**
- `equip` - Equip tool by name
- `unequip` - Unequip current tool
- `activate` - Activate equipped tool (attack, use)
- `deactivate` - Stop tool activation
- `interact` - Trigger nearby ProximityPrompt
- `jump` - Make character jump

#### bot_query_server

Execute Luau code on the game server (requires active playtest).

```json
{
  "code": "return game.Players:GetPlayers()[1].leaderstats.Coins.Value"
}
```

Use this to check:
- Player currency/stats
- DataStore values
- Server-side game state
- Service configurations

#### bot_wait_for

Wait for a condition to become true (polling-based).

```json
{
  "condition": "workspace:FindFirstChild('SpawnedItem') ~= nil",
  "timeout": 10,
  "context": "server"
}
```

**Parameters:**
- `condition` - Luau code returning boolean
- `timeout` - Max wait in seconds (default: 30)
- `poll_interval` - Check interval in ms (default: 100)
- `context` - "server" or "client" (default: server)

## Example Workflows

### Testing a Shop System

**Goal:** Verify player can buy an item from shop.

```
1. bot_observe type="find" query="Shop"
   → Find shop location

2. bot_move objectName="ShopNPC"
   → Navigate to shop

3. bot_action action="interact"
   → Open shop UI

4. bot_query_server code="return player.leaderstats.Coins.Value"
   → Record initial coins

5. bot_command type="ui" command="clickButton" args={"path": "ShopUI.BuyButton"}
   → Purchase item

6. bot_wait_for condition="player.Backpack:FindFirstChild('HealthPotion')"
   → Wait for item to appear

7. bot_observe type="inventory"
   → Verify item in inventory

8. bot_query_server code="return player.leaderstats.Coins.Value"
   → Verify coins decreased
```

### Testing Combat

**Goal:** Verify weapon deals damage to enemies.

```
1. bot_observe type="inventory"
   → Check available weapons

2. bot_action action="equip" name="Sword"
   → Equip weapon

3. bot_observe type="npcs" radius=50
   → Find nearby enemies

4. bot_move objectName="TrainingDummy"
   → Navigate to target

5. bot_action action="activate"
   → Attack

6. bot_wait_for condition="workspace.TrainingDummy.Humanoid.Health < 100"
   → Wait for damage

7. bot_query_server code="return workspace.TrainingDummy.Humanoid.Health"
   → Verify damage dealt
```

### Testing Checkpoint System

**Goal:** Verify checkpoints save player position.

```
1. bot_observe type="state"
   → Get spawn position

2. bot_move objectName="Checkpoint1"
   → Navigate to checkpoint

3. bot_action action="interact"
   → Activate checkpoint

4. run_code code="game.Players:GetPlayers()[1]:LoadCharacter()"
   → Force respawn

5. bot_wait_for condition="game.Players:GetPlayers()[1].Character ~= nil"
   → Wait for respawn

6. bot_observe type="state"
   → Verify position near checkpoint (not original spawn)
```

## Example Claude Prompts

These prompts demonstrate how to instruct AI agents for testing:

### Feature Testing

> "Test that the player can buy a sword from the shop. Navigate to the shop NPC, interact with it, purchase the sword, and verify it appears in inventory."

### Regression Testing

> "Verify the checkpoint system saves player position correctly. Touch a checkpoint, die, and confirm you respawn at the checkpoint instead of the original spawn point."

### Error Detection

> "Run a 60-second playtest and report any errors or warnings in the console. Pay special attention to nil errors or script timeouts."

### Integration Testing

> "Test the full onboarding flow: spawn in, collect the tutorial sword, defeat the practice dummy, and verify the quest completion popup appears."

### Performance Testing

> "Check for errors when loading a large inventory. Use run_code to give the player 100 items, then verify the inventory UI displays correctly."

## HTTP Requirement for Bot Testing

Interactive bot testing tools communicate with the game during playtests via HTTP. This requires enabling HTTP requests in your Roblox game settings.

### Which Tools Need HTTP?

| Tool | Requires HTTP | Notes |
|------|---------------|-------|
| `run_test` | ❌ No | Uses script injection, works without HTTP |
| `run_code` | ❌ No | Executes in plugin context, no HTTP needed |
| `sync_to_studio` | ❌ No | Uses plugin API |
| `bot_observe` | ✅ Yes | Queries game state via HTTP |
| `bot_move` | ✅ Yes | Sends movement commands via HTTP |
| `bot_action` | ✅ Yes | Sends action commands via HTTP |
| `bot_query_server` | ✅ Yes | Executes server-side code via HTTP |
| `bot_wait_for` | ✅ Yes | Polls conditions via HTTP |
| `bot_command` | ✅ Yes | Sends generic commands via HTTP |

### How to Enable HTTP Requests

1. In Roblox Studio, go to **Home** → **Game Settings** (or **File** → **Game Settings**)
2. Navigate to the **Security** tab
3. Enable **Allow HTTP Requests**
4. Click **Save**

::: warning Published Games
For published games, this setting affects both Studio testing and the live game. HTTP requests are disabled by default for security. Only enable if you understand the implications.
:::

### Why HTTP is Required

During a playtest, the bot tools need to communicate with the running game server to:
- Read player state (position, health, inventory)
- Send movement and action commands
- Execute server-side Luau code
- Poll for condition changes

The RbxSync plugin creates HTTP endpoints during playtests that the bot tools call. Without HTTP enabled, these requests will fail with connection errors.

### Troubleshooting HTTP Issues

If bot commands fail with "HTTP request failed" or similar errors:

1. **Verify HTTP is enabled** - Check Game Settings → Security
2. **Check firewall settings** - Ensure localhost connections are allowed
3. **Verify playtest is running** - HTTP endpoints only exist during playtests
4. **Check plugin connection** - Plugin must show green indicator

## Limitations

### Plugin Context vs Game Context

- `run_code` executes in **plugin context** (edit mode)
  - Can modify instances, spawn items, set properties
  - Cannot access runtime game services directly

- `bot_query_server` executes in **game server context** (play mode)
  - Can read player stats, DataStores, runtime state
  - Requires active playtest

### No Direct Input Simulation

Roblox's `VirtualInputManager` is internal-only. The bot system uses programmatic APIs instead:

- `Humanoid:MoveTo()` instead of WASD
- `Tool:Activate()` instead of mouse click
- `ProximityPrompt:InputHoldBegin()` instead of E key

This means some input-dependent features may not be testable.

### Polling-Based Observation

State observation uses polling (not events). For optimal performance:
- Limit `bot_observe` calls to ~10/second
- Use `bot_wait_for` for async conditions
- Batch queries when possible

### Pathfinding Constraints

- Maximum efficient path: ~500 studs before recompute
- Cannot climb TrussParts by default
- May get stuck on complex geometry (has stuck detection)

## Troubleshooting

### "Bot command timeout"

**Cause:** No playtest running or plugin disconnected.

**Fix:**
1. Ensure playtest is active (F5 in Studio)
2. Verify plugin shows green connection indicator
3. Check `rbxsync serve` is running

### "No character found"

**Cause:** Character hasn't spawned yet.

**Fix:**
- Add delay after starting playtest
- Use `bot_wait_for` with character spawn condition:
  ```json
  {"condition": "game.Players:GetPlayers()[1].Character ~= nil"}
  ```

### Movement not reaching destination

**Cause:** Obstacles blocking path or destination unreachable.

**Fix:**
1. Use `bot_observe type="nearby"` to debug surroundings
2. Try moving to a nearby accessible point first
3. Check for dynamic obstacles spawned after path computation

### "Server query failed"

**Cause:** Code error or LoadStringEnabled not set.

**Fix:**
1. Verify Luau syntax is correct
2. Check Studio output for `[RbxSync] loadstring available`
3. Ensure playtest is running (not just edit mode)

### UI interactions failing

**Cause:** UI path incorrect or element not visible.

**Fix:**
1. Use `bot_observe type="state"` to see `visibleUI` list
2. Verify exact path: `ScreenGui.Frame.Button`
3. Ensure UI is enabled and visible before clicking

## Best Practices

1. **Observe before acting** - Always check state before issuing commands
2. **Use wait_for for async** - Don't assume instant state changes
3. **Handle failures gracefully** - Check return values, implement retries
4. **Keep tests focused** - One behavior per test for clear debugging
5. **Log test progress** - Use `run_code` to print test markers
6. **Test incrementally** - Verify each step before combining into flows

## Related Documentation

- [Bot Testing Reference](/bot-testing) - Detailed API reference for bot tools
- [E2E Testing in VS Code](/vscode/e2e-testing) - VS Code-specific testing features
- [MCP Tools](/mcp/tools) - Complete MCP tool reference
- [MCP Setup](/mcp/setup) - Configuring MCP clients
