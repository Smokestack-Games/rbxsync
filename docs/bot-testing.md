# Bot Testing: AI-Powered Automated Gameplay Testing

RbxSync's bot testing system enables AI agents to control characters in Roblox Studio playtests - similar to Baritone for Minecraft. This allows thorough, automated testing of game features in real context.

## Overview

The bot testing system consists of:

1. **BotController.luau** - Luau module for character control (pathfinding, actions, UI)
2. **HTTP Endpoints** - `/bot/*` endpoints on the RbxSync server
3. **MCP Tools** - `bot_observe`, `bot_move`, `bot_action`, `bot_command`

## Quick Start

### Prerequisites

1. RbxSync server running (`rbxsync serve`)
2. RbxSync plugin installed in Studio
3. A game open in Studio with a spawn point
4. **HTTP Requests enabled** (for bot commands)
   - Go to **Game Settings** → **Security** → Enable **Allow HTTP Requests**
   - Required for: `bot_observe`, `bot_move`, `bot_action`, `bot_query_server`
   - NOT required for: `run_test` (uses script injection)

### Basic Usage

1. **Start an automated playtest**:
   ```
   run_test duration=30
   ```
2. **Or manually** press F5 in Studio, then use bot commands
2. **Observe game state**:
   ```
   bot_observe type="state"
   ```
3. **Move character**:
   ```
   bot_move objectName="ShopNPC"
   ```
4. **Perform actions**:
   ```
   bot_action action="interact"
   ```

## MCP Tools

### run_test

Run an automated playtest in Studio and capture all console output.

**Parameters:**
- `duration` - Test duration in seconds (default: 5)
- `mode` - Test mode: `Play` or `Run` (default: Play)

**Examples:**
```json
// Run a 30 second playtest
{"duration": 30, "mode": "Play"}
```

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
[1.95s] [MovementTest] Starting movement verification...
[2.46s] [MovementTest] Jump result: SUCCESS
[5.05s] [MovementTest] Walk: PASS
```

**Built-in Movement Test:**
When a playtest starts, the bot automatically runs a movement verification test:
- Tests jumping (humanoid.Jump)
- Tests walking (humanoid:MoveTo)
- Reports PASS/FAIL for each

### bot_observe

Get current game state during a playtest.

**Parameters:**
- `type` - Observation type: `state`, `nearby`, `npcs`, `inventory`, `find`
- `radius` - Search radius in studs (for `nearby`, `npcs`)
- `query` - Search query (for `find`)

**Examples:**
```json
// Get full state
{"type": "state"}

// Find nearby objects
{"type": "nearby", "radius": 30}

// Find NPCs
{"type": "npcs", "radius": 50}

// Get inventory
{"type": "inventory"}

// Search for objects
{"type": "find", "query": "Shop"}
```

**Returns:**
```json
{
  "position": {"x": 0, "y": 5, "z": 0},
  "health": 100,
  "maxHealth": 100,
  "inventory": ["Sword", "Health Potion"],
  "equipped": "Sword",
  "nearbyObjects": [...],
  "nearbyNPCs": [...],
  "visibleUI": ["ShopUI"],
  "isMoving": false,
  "currentAction": null
}
```

### bot_move

Move character to a position or named object using PathfindingService.

**Parameters:**
- `position` - Target position as `{x, y, z}` (use this OR objectName)
- `objectName` - Name of object to navigate to (use this OR position)

**Examples:**
```json
// Move to coordinates
{"position": {"x": 50, "y": 5, "z": 30}}

// Move to named object
{"objectName": "ShopNPC"}
```

**Returns:**
```json
{
  "success": true,
  "reached": true,
  "finalPosition": {"x": 49.5, "y": 5, "z": 30.2},
  "pathLength": 45.3
}
```

### bot_action

Perform character actions like equipping tools, interacting, jumping.

**Parameters:**
- `action` - Action type: `equip`, `unequip`, `activate`, `deactivate`, `interact`, `jump`
- `name` - Tool or object name (for `equip`, `interact`)

**Examples:**
```json
// Equip a tool
{"action": "equip", "name": "Sword"}

// Activate equipped tool (attack/use)
{"action": "activate"}

// Interact with nearby object
{"action": "interact", "name": "ShopNPC"}

// Jump
{"action": "jump"}
```

### bot_command

Send generic commands for advanced control.

**Parameters:**
- `type` - Command category: `move`, `action`, `ui`, `observe`
- `command` - Specific command name
- `args` - Command arguments

**UI Commands:**
```json
// Click a button
{"type": "ui", "command": "clickButton", "args": {"path": "ShopUI.BuyButton"}}

// Read text
{"type": "ui", "command": "readText", "args": {"path": "ShopUI.PriceLabel"}}

// Fill text box
{"type": "ui", "command": "fillTextBox", "args": {"path": "ChatUI.Input", "text": "Hello"}}
```

## Test Scenarios

Test scenarios are Lua files in `testing/bot-tests/` that define automated tests.

### Scenario Structure

```lua
local test = {
    scenario_name = "shop_purchase_test",
    description = "Test buying an item from shop",

    -- Setup runs before playtest
    setup = function()
        -- Create test fixtures, set player state, etc.
    end,

    -- Goal for AI (natural language)
    goal = "Navigate to shop, buy Health Potion, verify in inventory",

    -- Success criteria
    check = function(state)
        return state.inventory:contains("Health Potion")
    end,

    timeout = 60,

    -- Optional: explicit test steps
    steps = {
        { description = "Find shop", command = {...} },
        { description = "Navigate", command = {...} },
        -- ...
    }
}

return test
```

### Example: Navigation Test

```lua
local test = {
    scenario_name = "navigation_test",
    goal = "Navigate to the green target, then the blue target",

    setup = function()
        -- Create target parts
        local target1 = Instance.new("Part")
        target1.Name = "GreenTarget"
        target1.Position = Vector3.new(50, 5, 50)
        target1.Parent = workspace
    end,

    check = function(state)
        return state.reachedTargets["GreenTarget"]
            and state.reachedTargets["BlueTarget"]
    end,

    timeout = 60
}
```

### Example: Combat Test

```lua
local test = {
    scenario_name = "combat_test",
    goal = "Equip sword, find training dummy, attack until defeated",

    check = function(state)
        local dummy = workspace:FindFirstChild("TrainingDummy")
        return dummy.Humanoid.Health <= 0
    end,

    timeout = 120
}
```

## AI Decision Loop

The AI agent follows this loop during testing:

```
1. bot_observe() → Get current game state
2. Analyze state vs goal
3. Decide next action(s)
4. bot_move() / bot_action() → Execute
5. Wait for completion
6. Repeat until goal achieved or timeout
```

### Goal Decomposition Example

```
Goal: "Test the shop system"
  ├─ Find shop NPC location      → bot_observe type="find" query="Shop"
  ├─ Navigate to shop            → bot_move objectName="ShopNPC"
  ├─ Interact with shop NPC      → bot_action action="interact"
  ├─ Browse available items      → bot_command type="ui" command="getVisibleUI"
  ├─ Purchase item               → bot_command type="ui" command="clickButton"
  ├─ Verify inventory updated    → bot_observe type="inventory"
  └─ Check currency deducted     → bot_observe type="state"
```

## HTTP API

For direct HTTP access (advanced use):

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/bot/state` | GET | Get current game state |
| `/bot/move` | POST | Move character |
| `/bot/action` | POST | Perform action |
| `/bot/observe` | POST | Observe with options |
| `/bot/command` | POST | Generic command |

### Example cURL

```bash
# Get state
curl http://localhost:44755/bot/state

# Move to object
curl -X POST http://localhost:44755/bot/move \
  -H "Content-Type: application/json" \
  -d '{"objectName": "ShopNPC"}'

# Perform action
curl -X POST http://localhost:44755/bot/action \
  -H "Content-Type: application/json" \
  -d '{"action": "interact"}'
```

## Limitations

### Roblox API Constraints

- **VirtualInputManager** - NOT accessible (Roblox internal only)
- **Direct input simulation** - Cannot simulate keyboard/mouse
- **Workaround**: Use programmatic APIs (MoveTo, Tool:Activate, etc.)

### Pathfinding Limitations

- Max efficient path: ~500 studs before recompute
- Cannot climb TrussParts by default
- Humanoid can get stuck - has stuck detection

### Important Notes

- Bot commands only work during active playtests (F5/F6)
- The plugin must be connected to the server
- **HTTP Requests must be enabled** in Game Settings → Security for bot commands to work
- State observation should be throttled (~10/sec max)

## TestAssertions Module

The `TestAssertions.luau` module provides helper functions for writing game tests.

### Available Functions

```lua
local TestAssertions = require(path.to.TestAssertions)

-- Basic assertions
TestAssertions.assertEqual(actual, expected, "Values should match")
TestAssertions.assertTrue(condition, "Should be true")
TestAssertions.assertFalse(condition, "Should be false")
TestAssertions.assertNil(value, "Should be nil")
TestAssertions.assertNotNil(value, "Should not be nil")
TestAssertions.assertGreater(a, b, "a should be > b")
TestAssertions.assertLess(a, b, "a should be < b")

-- Async helpers
local success = TestAssertions.waitFor(function()
    return player:FindFirstChild("DataLoaded")
end, 10, 0.1) -- timeout, poll interval

-- Change detection
local changed = TestAssertions.expectChange(
    function() return player.Coins.Value end,  -- getValue
    function() buyItem() end,                   -- action
    function(before, after) return after < before end  -- predicate
)
```

### Example: Economy Test

```lua
local TestAssertions = require(ReplicatedStorage.TestAssertions)

-- Test that buying a ball costs coins
local function testBallPurchase()
    local initialCoins = player.leaderstats.Coins.Value

    -- Buy a ball
    GameService:BuyBalls(player, 1)

    -- Assert coins decreased
    TestAssertions.assertLess(
        player.leaderstats.Coins.Value,
        initialCoins,
        "Coins should decrease after purchase"
    )
end
```

### Example: Wait for State

```lua
-- Wait for player data to load
local loaded = TestAssertions.waitFor(function()
    return DataService:GetData(player) ~= nil
end, 5) -- 5 second timeout

if not loaded then
    error("Player data failed to load")
end
```

## Troubleshooting

### "Bot command timeout"

- Ensure playtest is running (F5 in Studio)
- Check plugin is connected (green indicator)
- Verify server is running (`rbxsync serve`)

### "No character found"

- Playtest not started or character not spawned
- Wait for character to spawn before issuing commands

### Movement not reaching destination

- Check for obstacles blocking path
- Increase timeout for long paths
- Use `bot_observe type="nearby"` to debug surroundings

### UI interactions failing

- Verify UI path is correct
- Ensure UI is visible before clicking
- Use `bot_command type="ui" command="getVisibleUI"` to debug

### "HTTP request failed" or connection errors

Bot commands communicate with the game via HTTP during playtests.

**Fix:**
1. Enable HTTP Requests: **Game Settings** → **Security** → **Allow HTTP Requests**
2. Ensure a playtest is running (F5 in Studio)
3. Verify plugin is connected (green indicator)

Note: `run_test` does not require HTTP - it uses script injection. Only the interactive `bot_*` commands need HTTP enabled.

## Best Practices

1. **Always observe first** - Get state before issuing commands
2. **Use goals, not scripts** - Let AI figure out steps when possible
3. **Handle failures gracefully** - Check return values, retry on failure
4. **Keep timeouts reasonable** - 30-60s for navigation, 5-10s for actions
5. **Test incrementally** - Verify each step before combining into full test
