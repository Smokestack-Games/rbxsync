# MCP Integration

RbxSync includes an MCP (Model Context Protocol) server for AI agent integration.

## Overview

The MCP server lets AI agents:
- Extract games to files
- Sync changes to Studio
- Run code in Studio
- Execute playtests
- See console output
- Manage git operations

## Quick Start

1. Build the MCP server:
   ```bash
   cargo build --release
   ```

2. Configure your MCP client (see [Setup](/mcp/setup))

3. Connect Studio to RbxSync

4. AI agents can now extract, sync, test, and debug your game

## Available Tools

| Tool | Description |
|------|-------------|
| `extract_game` | Extract game to files |
| `sync_to_studio` | Push changes to Studio |
| `run_code` | Execute Luau in Studio |
| `run_test` | Run playtest with output |
| `git_status` | Get repository status |
| `git_commit` | Commit changes |

See [Tools](/mcp/tools) for full reference.

## Use Cases

### Autonomous Development
AI writes code, runs playtests, sees errors, fixes them—iterating until it works.

### Code Review
Extract a game and let AI analyze patterns, suggest refactors, and catch bugs.

### Automated Testing
Run test suites programmatically and parse console output for pass/fail results.

### Live Debugging
AI sees runtime errors in real-time and can fix them on the spot.

## Next Steps

- [Setup](/mcp/setup) - Configure your MCP client
- [Tools](/mcp/tools) - Complete tool reference
