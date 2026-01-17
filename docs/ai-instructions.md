# AI Instructions for RbxSync

This guide explains how AI coding assistants can use RbxSync to develop, test, and debug Roblox games.

## Overview

RbxSync is the only Roblox sync tool with native AI integration. Unlike other tools (Rojo, Argon, Pesto), RbxSync includes:

- **MCP (Model Context Protocol)** tools for direct Studio control
- **Bot testing** capabilities for automated gameplay testing
- **Real-time console streaming** for error visibility
- **Two-way sync** so AI edits appear in Studio instantly

## Supported AI Tools

### Claude Code (Recommended)

Claude Code has full MCP integration. The RbxSync MCP server provides tools that Claude can call directly.

**Setup:**

1. Add to your MCP config (~/.config/claude/mcp.json or project .mcp.json):

```json
{
  "mcpServers": {
    "rbxsync": {
      "command": "/path/to/rbxsync-mcp"
    }
  }
}
```

2. Start the sync server:

```bash
rbxsync serve
```

3. Connect Roblox Studio via the RbxSync plugin

Claude Code now has access to all RbxSync MCP tools.

See [MCP Setup](/mcp/setup) for detailed configuration.

### Cursor

Cursor can use RbxSync through the .cursorrules file (already included in the repo).

**Setup:**

1. Open your RbxSync project in Cursor
2. The .cursorrules file provides context about the project
3. Use rbxsync serve for syncing
4. Edit files normally - Cursor works with the local .luau files

**Cursor workflow:**

```
1. Edit .luau files in Cursor
2. Changes auto-sync to Studio
3. Use "rbxsync extract" to pull Studio changes
4. Cursor sees the updated files
```

### Other AI Assistants (GitHub Copilot, Codeium, etc.)

Other AI tools work with RbxSync at the file level:

1. Edit .luau and .rbxjson files
2. Changes sync automatically via file watcher
3. Use llms.txt (in repo root) for project context

**Tip:** Copy the contents of llms.txt into your AI assistant's context to help it understand the project.

## MCP Tools Overview

When using Claude Code with RbxSync MCP, these tools are available:

| Tool | Description |
|------|-------------|
| extract_game | Extract game from Studio to files |
| sync_to_studio | Push local changes to Studio |
| run_code | Execute Luau code in Studio |
| run_test | Run playtest with console capture |
| bot_observe | Get game state during playtest |
| bot_move | Move character using pathfinding |
| bot_action | Perform character actions |
| bot_command | Generic bot command |
| bot_query_server | Execute server-side Luau |
| bot_wait_for | Wait for condition |
| git_status | Get repository status |
| git_commit | Commit changes |

See [MCP Tools Reference](/mcp/tools) for core tool details.

## Bot Testing Tools

These tools enable AI agents to control a character during playtests. All bot tools require an active playtest (call run_test first or press F5 manually).

### bot_observe

Get current game state during a playtest.

**Parameters:**
- type (optional, default: "state"): Observation type
  - "state": Full state (position, health, inventory, UI)
  - "nearby": Objects within radius
  - "npcs": NPCs within radius
  - "inventory": Character inventory/tools
  - "find": Search for specific objects
- radius (optional): Search radius in studs
- query (optional): Search query for "find" type

**Example prompts:**
- "What objects are near my character?"
- "Check the player's inventory"
- "Find all NPCs within 100 studs"

### bot_move

Move the character to a position or object using pathfinding.

**Parameters:**
- position (optional): Target position as {x, y, z}
- objectName (optional): Name of object to move to

Use one of position OR objectName, not both.

**Example prompts:**
- "Move to position 10, 5, 30"
- "Walk to the object named 'Shop'"
- "Navigate to the SpawnLocation"

### bot_action

Perform character actions.

**Parameters:**
- action (required): Action type
  - "equip": Equip a tool from inventory
  - "unequip": Unequip current tool
  - "activate": Activate equipped tool (like clicking)
  - "deactivate": Stop activation
  - "interact": Interact with nearby object
  - "jump": Make character jump
- name (optional): Tool or object name (for equip, interact)

**Example prompts:**
- "Equip the Sword"
- "Interact with the door"
- "Make the character jump"

### bot_command

Send generic bot commands for advanced control.

**Parameters:**
- type (required): Command type (move, action, ui, observe)
- command (required): Specific command name
- args (optional): Command arguments as JSON

**Example:**
```json
{
  "type": "ui",
  "command": "clickButton",
  "args": {"name": "PlayButton"}
}
```

### bot_query_server

Execute Luau code on the game server during a playtest.

**Parameters:**
- code (required): Luau code to execute on server

**Example prompts:**
- "Query the player's coins: game.Players:GetPlayers()[1].leaderstats.Coins.Value"
- "Count how many players: #game.Players:GetPlayers()"

**Use cases:**
- Check server-side game state
- Query DataStore values
- Inspect leaderstats/currency

### bot_wait_for

Wait for a condition to become true during playtest.

**Parameters:**
- condition (required): Luau code that returns boolean
- timeout (optional, default: 30): Maximum wait in seconds
- poll_interval (optional, default: 100): Poll interval in milliseconds
- context (optional, default: "server"): "server" or "client"

**Example prompts:**
- "Wait until the Ball object is destroyed"
- "Wait for the shop UI to open"

## Workflow Examples

### Example 1: Bug Fix Workflow

```
User: "There's a bug where players fall through the floor"

AI workflow:
1. extract_game to get current code
2. Read the relevant scripts
3. Find the bug
4. Edit the .luau file
5. sync_to_studio to push fix
6. run_test with duration: 5 to verify
7. If errors, iterate
8. git_commit when done
```

### Example 2: Feature Development

```
User: "Add a coin collection system"

AI workflow:
1. extract_game to understand current structure
2. Create CoinService.luau in ServerScriptService
3. Create Coin.rbxjson model template
4. sync_to_studio
5. run_test to verify no errors
6. Use bot_observe to check if coins appear
7. Use bot_move to walk to a coin
8. Use bot_query_server to verify coin count increases
```

### Example 3: Automated Testing

```
User: "Test that the shop works correctly"

AI workflow:
1. run_test with duration: 60
2. bot_observe type: "state" to get initial state
3. bot_move to objectName: "Shop"
4. bot_action action: "interact" name: "Shop"
5. bot_observe type: "state" to see UI
6. bot_command type: "ui" command: "clickButton" args: {name: "BuyButton"}
7. bot_query_server to check if item was purchased
8. bot_wait_for condition: "player.Inventory:FindFirstChild('Sword')"
```

### Example 4: Debugging with Console

```
User: "Why does my game lag?"

AI workflow:
1. run_test with duration: 10
2. Check console output for warnings
3. run_code: "print(#workspace:GetDescendants())" to count instances
4. run_code: "print(Stats.HeartbeatTimeMs)" to check performance
5. Analyze results and suggest fixes
```

## Best Practices

### For AI Agents

1. **Always extract before major changes** - Ensures you have the latest code
2. **Use incremental sync** - sync_to_studio only sends changed files
3. **Check console output** - run_test captures all prints/warnings/errors
4. **Test after changes** - Run a quick playtest to verify no errors
5. **Use bot tools for verification** - Automated testing catches issues

### For Users

1. **Keep Studio connected** - The plugin must be active for MCP tools
2. **Enable console streaming** - Toggle E2E mode in VS Code for real-time output
3. **Use version control** - Commit regularly so AI can understand change history
4. **Provide context** - Tell the AI what services/scripts are relevant

## Troubleshooting

### "Not connected to RbxSync server"

The MCP server cannot reach the sync server.

**Fix:**
1. Run rbxsync serve
2. Check port 44755 is not blocked
3. Ensure Studio plugin is connected

### "loadstring not available"

The run_code tool requires loadstring.

**Fix:**
- This should work automatically for plugins
- Check Studio output for "[RbxSync] loadstring available"

### "Bot command failed"

Bot tools require an active playtest.

**Fix:**
1. Call run_test first, OR
2. Press F5 in Studio manually
3. Then use bot tools

### Sync not working

Changes not appearing in Studio.

**Fix:**
1. Check plugin shows "Connected"
2. Verify "Files -> Studio" is enabled in plugin
3. Check the sync direction (arrows in plugin UI)

## File References

- llms.txt - AI discovery file with project overview
- .cursorrules - Cursor-specific context and rules
- CLAUDE.md - Detailed agent instructions for Claude
- README.md - Full project documentation

## Related Documentation

- [MCP Overview](/mcp/) - Introduction to MCP integration
- [MCP Setup](/mcp/setup) - Configuration guide
- [MCP Tools](/mcp/tools) - Core tool reference
- [Bot Testing](/bot-testing) - Detailed bot testing guide

## Version

This documentation applies to RbxSync v1.3.0 and later.

## Links

- GitHub: https://github.com/devmarissa/rbxsync
- Documentation: https://rbxsync.dev/docs
- VS Code Extension: https://marketplace.visualstudio.com/items?itemName=rbxsync.rbxsync
- Studio Plugin: https://create.roblox.com/store/asset/89280418878393/RbxSync
