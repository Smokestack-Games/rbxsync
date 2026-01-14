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

## extract_game

Extract the connected game to files.

**Input:**
```json
{
  "project_dir": "/Users/you/MyGame",  // required - where to extract files
  "services": ["Workspace", "ReplicatedStorage"]  // optional - specific services to extract
}
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `project_dir` | string | Yes | Directory to extract files to (must have rbxsync.json) |
| `services` | string[] | No | Specific services to extract (defaults to all) |

**Output:**
```json
{
  "success": true,
  "instances": 1247,
  "path": "/Users/you/MyGame/src"
}
```

## sync_to_studio

Push local file changes to Studio.

**Input:**
```json
{
  "path": "/Users/you/MyGame"  // optional
}
```

**Output:**
```json
{
  "success": true,
  "changes": 5
}
```

## run_code

Execute Luau code in Studio.

**Input:**
```json
{
  "code": "print('Hello from MCP!')"
}
```

**Output:**
```json
{
  "success": true,
  "output": "Hello from MCP!"
}
```

## run_test

Run a playtest with console output capture.

**Input:**
```json
{
  "duration": 10,  // seconds (default: 5)
  "mode": "Play"   // "Play" or "Run" (default: "Play")
}
```

**Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `duration` | number | 5 | How long to run the test in seconds |
| `mode` | string | "Play" | "Play" for solo playtest (F5), "Run" for server simulation |

**Output:**
```json
{
  "success": true,
  "output": [
    { "type": "info", "message": "Game started" },
    { "type": "warn", "message": "Low memory" },
    { "type": "error", "message": "Script error" }
  ]
}
```

## stop_test

Stop the current playtest.

**Input:**
```json
{}
```

**Output:**
```json
{
  "success": true
}
```

## git_status

Get the git status of the project.

**Input:**
```json
{
  "path": "/Users/you/MyGame"  // optional
}
```

**Output:**
```json
{
  "branch": "main",
  "staged": ["src/ServerScriptService/Main.server.luau"],
  "modified": ["src/ReplicatedStorage/Config.luau"],
  "untracked": []
}
```

## git_commit

Commit staged changes.

**Input:**
```json
{
  "message": "Add player spawning logic",
  "path": "/Users/you/MyGame"  // optional
}
```

**Output:**
```json
{
  "success": true,
  "commit": "abc1234"
}
```

## Example Workflow

Here's how an AI might use these tools:

1. **Extract current state**
   ```json
   { "tool": "extract_game" }
   ```

2. **Modify code** (using file tools)

3. **Sync changes**
   ```json
   { "tool": "sync_to_studio" }
   ```

4. **Run test**
   ```json
   { "tool": "run_test", "arguments": { "mode": "play" } }
   ```

5. **Check output for errors**

6. **Fix and repeat** until tests pass

7. **Commit changes**
   ```json
   { "tool": "git_commit", "arguments": { "message": "Fix player spawn bug" } }
   ```
