# Error Message Review Report

**Date:** 2026-01-16
**Scope:** All user-facing error messages in RbxSync

---

## Summary

This report catalogs user-facing error messages across all RbxSync components:
- **Rust CLI/Server:** 70+ error messages
- **Luau Plugin:** 40+ warning/error messages
- **VS Code Extension:** 25+ error messages

Overall assessment: Many errors are technical and actionable, but several lack context or suggest fixes.

---

## 1. Rust Error Messages (CLI, Server, Core)

### 1.1 CLI Errors (`rbxsync-cli/src/main.rs`)

| Line | Error Message | Assessment | Suggested Improvement |
|------|---------------|------------|----------------------|
| 599 | `"Place file not found: {}"` | Good - shows path | Add: "Check that the file exists and path is correct" |
| 606 | `"Invalid place file format. Expected .rbxl or .rbxlx"` | Good - clear and actionable | - |
| 754 | Multi-line version mismatch warning | Excellent - very helpful | - |
| 786 | `"Roblox Studio not found. Please install it from roblox.com"` | Good - actionable | Could link directly to download page |
| 791 | `"Roblox Studio is not available on this platform"` | Good - clear | - |
| 942-946 | JSON parse error with context | Excellent - shows error, explains fix | - |
| 953-960 | Port in use error with suggestions | Excellent - troubleshooting steps | - |
| 1566, 1733, 2276 | `"Source directory not found: {}"` | Good but repeated | Unify to single location |
| 1743 | `"Unknown format: {}. Use rbxl, rbxm, rbxlx, or rbxmx"` | Good - lists valid options | - |
| 2452 | `"Failed to clone repository"` | Vague | Should include git error output |
| 2495 | `"Failed to build CLI"` | Vague | Should include build error output |

### 1.2 Server Errors (`rbxsync-server/src/lib.rs`)

| Line | Error Message | Assessment | Suggested Improvement |
|------|---------------|------------|----------------------|
| 602 | `"Empty workspace directory"` | Unclear | "No workspace folder open. Please open a folder in VS Code first." |
| 704 | `"Empty project directory"` | Unclear | "Project directory not specified. Make sure rbxsync.json exists." |
| 726 | `"No Studio instances connected to update"` | Good | Add: "Start Studio and ensure the plugin is installed" |
| 817, 866 | `"No Studio found with place_id {}"` | Technical | "Studio with place ID {} is not connected. Make sure the correct place is open." |
| 893 | `"Session not found"` | Vague | "Extraction session expired or not started. Try extracting again." |
| 914 | `"No backup found to restore"` | Good | Could add: "Backup is only available immediately after extraction" |
| 923, 934 | `"Failed to remove/restore current src: {}"` | Technical | Keep technical but add: "Check file permissions" |
| 1306 | `"No active extraction session"` | Good | - |
| 1391 | `"No extraction data available"` | Vague | "Extraction has not completed yet or failed to gather data" |
| 1414 | `"No extraction session active"` | Good | - |
| 1894, 1937, 1948 | Terrain directory/file errors | Technical | Add context about what terrain extraction requires |
| 2027, 2034 | `"Channel closed"` / `"Timeout waiting for plugin response"` | Very technical | "Lost connection to Studio. Try reconnecting." |
| 2139, 2356, 2633, 2942 | `"Source directory does not exist"` | Good but repeated | Unify and suggest creating with `rbxsync init` |
| 3291 | `"Plugin response timeout - make sure Studio is connected"` | Good - actionable | - |
| 3503 | `"Bot command timeout - ensure playtest is running"` | Good - actionable | - |
| 3673 | `"Missing or invalid command ID"` | Technical | User shouldn't see this - internal error |
| 3774 | `"Unknown lifecycle event: {}"` | Technical | User shouldn't see this - internal error |

### 1.3 Context Errors (`.context()` usage)

Generally good - provides context for underlying errors:

| File | Context Message | Assessment |
|------|-----------------|------------|
| `plugin_builder.rs:68` | `"No entry point found. Expected init.server.luau"` | Excellent - specific |
| `plugin_builder.rs:75` | `"Failed to create output directory"` | Good |
| `main.rs:476-478` | `"Failed to create src/assets/terrain directory"` | Good |
| `main.rs:626` | `"Failed to launch Roblox Studio"` | Good |
| `main.rs:939` | `"Failed to read rbxsync.json"` | Could suggest: "Run rbxsync init first" |
| `main.rs:1015` | `"Invalid port number"` | Good |
| `imessage.rs:76` | `"Failed to open Messages database. You may need to grant Full Disk Access..."` | Excellent - actionable |

### 1.4 Typed Errors (`thiserror`)

**Rojo Errors (`rbxsync-core/src/rojo.rs`):**

| Error | Message | Assessment |
|-------|---------|------------|
| `IoError` | `"Failed to read Rojo project file: {0}"` | Good |
| `ParseError` | `"Failed to parse Rojo project file: {0}"` | Good |
| `NotFound` | `"Rojo project file not found: {0}"` | Good |

**Wally Errors (`rbxsync-core/src/types/wally.rs`):**

| Error | Message | Assessment |
|-------|---------|------------|
| `IoError` | `"Failed to read file: {0}"` | Generic - could be more specific |
| `TomlError` | `"Failed to parse TOML: {0}"` | Good |
| `ManifestNotFound` | `"Wally manifest not found at {0}"` | Good |
| `LockNotFound` | `"Wally lock file not found at {0}"` | Good |

---

## 2. Luau Plugin Errors (`plugin/src/`)

### 2.1 Sync.luau

| Line | Message | Assessment | Suggested Improvement |
|------|---------|------------|----------------------|
| 475 | `"[Sync.createInstance] Missing className in data"` | Debug - good | - |
| 529 | `"[RbxSync] Union '{}' not separable: {}"` | Good | Could explain why unions fail to separate |
| 555, 660 | `"[Sync.createInstance] Failed to create..."` | Good for debugging | - |
| 809 | `"[RbxSync] Cannot set {}.{}: {} (value type: {})"` | Excellent - detailed | - |
| 1368 | `"[RbxSync] Unknown material: {}"` | Good | Could list valid materials |
| 1376 | `"[RbxSync] MaterialVariant not found: {}"` | Good | - |
| 1440 | `"[RbxSync] Missing {} parts for union '{}', skipping"` | Good | - |
| 1549 | `"[RbxSync] Failed to reconstruct union '{}'"` | Vague | Should include why it failed |

### 2.2 init.server.luau (Main Plugin)

| Line | Message | Assessment | Suggested Improvement |
|------|---------|------------|----------------------|
| 584 | `"No project set. Enter path above or pass project_dir in command."` | Good - actionable | - |
| 608 | `"Can't load API. Try reopening Studio."` | Good - actionable | - |
| 829 | `"[RbxSync] Could not separate union '{}': {}"` | Good | - |
| 878 | `"Extraction failed. Try again."` | Vague | Should include specific error reason |
| 907 | `"{}. {}. Try again."` | Dynamic - depends on context | - |
| 1329 | `"Failed: {} - {}"` | Good - shows path and error | - |
| 1363 | `"[RbxSync] Sync error: {}"` | Generic | Could categorize error types |
| 1697 | `"[RbxSync] Failed to send response: {}"` | Technical | User shouldn't see this |
| 2269, 3504 | `"Could not connect to server"` | Clear but vague | "Could not connect to rbxsync server. Is it running? Try: rbxsync serve" |
| 3786-3790 | Multiple VS Code workspaces warning | Excellent - very helpful | - |

### 2.3 ChangeTracker.luau

| Line | Message | Assessment | Suggested Improvement |
|------|---------|------------|----------------------|
| 375 | `"[RbxSync] Sync batch {}/{} failed"` | Good - shows progress | - |
| 386 | `"[RbxSync] Sync sent {} changes but no files written - check project path"` | Excellent - actionable | - |
| 390-392 | Sync errors with list | Good | - |
| 675 | `"[RbxSync] Cannot flush queue - not connected"` | Good | Could suggest reconnection |
| 724 | `"[RbxSync] Some flush batches failed"` | Vague | Should list which batches |

### 2.4 Other Plugin Files

| File | Line | Message | Assessment |
|------|------|---------|------------|
| `Config.luau` | 38, 66, 116, 135 | `"[RbxSync] Config not initialized with plugin reference"` | Internal error - user shouldn't see |
| `TestRunner.luau` | 185 | `"[TestRunner] Recovering from stale test state"` | Debug info |
| `CSGHandler.luau` | 166, 180 | Union/SubtractAsync failed | Good |
| `Reflection.luau` | 56, 67, 98 | API dump errors | Good - fallback handling |
| `TerrainHandler.luau` | 176, 450 | Terrain chunk/color errors | Good |

---

## 3. VS Code Extension Errors (`rbxsync-vscode/src/`)

### 3.1 User-Facing Errors (showErrorMessage)

| File | Line | Message | Assessment | Suggested Improvement |
|------|------|---------|------------|----------------------|
| `sync.ts` | 11 | `"Not connected. Is Studio running?"` | Good - question format | Could add: "Click 'Connect' or run rbxsync serve" |
| `sync.ts` | 20 | `"Open a folder first."` | Brief but clear | - |
| `sync.ts` | 29 | `"Failed to read files."` | Vague | Should include which files or why |
| `sync.ts` | 136 | `"Sync failed. Try again."` | Too vague | Should include specific error |
| `test.ts` | 36 | `"Not connected. Is Studio running?"` | Duplicate of sync.ts:11 | Unify into shared constant |
| `test.ts` | 117 | `"Test failed: {}"` | Good - includes reason | - |
| `connect.ts` | 79 | `"Failed to start server. Check the terminal for errors."` | Good - directs user | - |
| `client.ts` | 144 | `"Failed to update Studio project path. Make sure Studio is connected."` | Good | - |
| `client.ts` | 170 | `"No workspace folder open"` | Clear | - |
| `client.ts` | 192, 196 | `"Failed to link studio"` | Vague | Should include why linking failed |
| `client.ts` | 220, 224 | `"Failed to unlink studio"` | Vague | Should include reason |
| `client.ts` | 241 | `"Failed to undo extraction"` | Good - dynamic error | - |
| `client.ts` | 245 | `"Failed to undo extraction - no backup found"` | Good - specific | - |
| `extract.ts` | 17 | `"Not connected. Is Studio running?"` | Duplicate | Unify |
| `extract.ts` | 26 | `"Open a folder first."` | Duplicate | Unify |
| `trash.ts` | 239 | `"No workspace folder open"` | Duplicate | Unify |
| `trash.ts` | 326 | `"Failed to recover: {}"` | Good - includes error | - |

### 3.2 Console Errors (for debugging)

These are appropriate for console logging but users may see them in developer tools:

| File | Message | Assessment |
|------|---------|------------|
| `projectJson.ts:151` | `"Failed to write project.json"` | Good |
| `server.ts:58` | `"Failed to initialize API dump"` | Technical |
| `server.ts:125, 146` | `"Completion/Hover error"` | LSP-specific |
| `client.ts:444` | `"RbxSync: {} failed: {}"` | Good format |
| `apiDump.ts:67` | `"Failed to fetch API dump"` | Technical |

---

## 4. Missing Error Cases

### 4.1 Not Handled / Silent Failures

1. **Network timeout during extraction** - No clear message when Studio connection times out mid-extraction
2. **Corrupt rbxsync.json** - Handled but could suggest auto-repair
3. **Permission denied on file write** - Generic IO error, could detect specifically
4. **Disk full during extraction** - No specific handling
5. **Invalid Roblox API dump** - Falls back silently, should warn user
6. **Plugin version mismatch** - No warning when CLI and plugin versions differ
7. **Git operations without git installed** - Could fail confusingly

### 4.2 Race Conditions / Concurrent Access

1. **Multiple Studio instances** - Good warning exists, but could be clearer about which to use
2. **File watcher conflicts** - No user-visible errors when file watching fails
3. **Simultaneous extractions** - Could warn if extraction is already in progress

---

## 5. Recommendations

### 5.1 High Priority Fixes

1. **Unify duplicate error messages** - "Not connected. Is Studio running?" appears 3+ times
2. **Add actionable suggestions** to vague errors like "Sync failed. Try again."
3. **Include error codes** for common issues to help with troubleshooting
4. **"Could not connect to server"** should suggest checking if server is running

### 5.2 Medium Priority

1. **Create error constants file** in each component for consistency
2. **Add structured logging** with error categories (CONNECTION, FILE_IO, PLUGIN, etc.)
3. **Improve Union errors** - CSG failures are common and confusing for users
4. **Version mismatch detection** between CLI, server, and plugin

### 5.3 Low Priority / Nice to Have

1. **Error telemetry** (opt-in) to understand common failure modes
2. **Help links** in error messages pointing to documentation
3. **Auto-recovery suggestions** for recoverable errors
4. **Localization** support for error messages

---

## 6. Error Message Style Guide (Proposed)

For consistency, recommend following these patterns:

```
Good:
- "[RbxSync] Cannot set Part.Color: expected Color3, got string"
- "Source directory not found: /path/to/src. Run 'rbxsync init' to create it."
- "Port 3000 is in use. Try: rbxsync serve --port 3001"

Avoid:
- "Error" (too vague)
- "Failed" (without reason)
- "Operation failed. Try again." (no debugging info)
- Technical jargon without context
```

---

## Appendix: Full Error Message Catalog

<details>
<summary>Click to expand full list</summary>

### CLI `bail!` / `anyhow!` Errors
- `"Place file not found: {}"`
- `"Invalid place file format. Expected .rbxl or .rbxlx"`
- `"Roblox Studio not found. Please install it from roblox.com"`
- `"Roblox Studio is not available on this platform"`
- `"Source directory not found: {}"`
- `"Unknown format: {}. Use rbxl, rbxm, rbxlx, or rbxmx"`
- `"Failed to clone repository"`
- `"Failed to build CLI"`
- `"Anthropic API error: {}"`
- `"AppleScript error: {}"`

### Server JSON Errors
- `"Empty workspace directory"`
- `"Empty project directory"`
- `"No Studio instances connected to update"`
- `"No Studio found with place_id {}"`
- `"Session not found"`
- `"No backup found to restore"`
- `"Failed to remove current src: {}"`
- `"Failed to restore from backup: {}"`
- `"No active extraction session"`
- `"No extraction data available"`
- `"No extraction session active"`
- `"Failed to create terrain directory: {}"`
- `"Failed to serialize terrain: {}"`
- `"Failed to write terrain file: {}"`
- `"Channel closed"`
- `"Timeout waiting for plugin response"`
- `"Source directory does not exist"`
- `"Failed to parse terrain data: {}"`
- `"Failed to read terrain file: {}"`
- `"Plugin returned error"`
- `"Timeout waiting for Studio paths"`
- `"Channel closed unexpectedly"`
- `"Plugin response timeout - make sure Studio is connected"`
- `"Bot command timeout - ensure playtest is running"`
- `"Missing or invalid command ID"`
- `"Unknown lifecycle event: {}"`

### Plugin Warnings
- `"[Sync.createInstance] Missing className in data"`
- `"[RbxSync] Union '{}' not separable: {}"`
- `"[Sync.createInstance] Failed to create CSG part"`
- `"[Sync.createInstance] Failed to create {}"`
- `"[RbxSync] Cannot set {}.{}: {}"`
- `"[RbxSync] Unknown material: {}"`
- `"[RbxSync] MaterialVariant not found: {}"`
- `"[RbxSync] Failed to set override {} -> {}: {}"`
- `"[RbxSync] CSG part not found: {}"`
- `"[RbxSync] Missing {} parts for union '{}', skipping"`
- `"[RbxSync] No add parts for union '{}', skipping"`
- `"[RbxSync] Failed to reconstruct union '{}'"`
- `"[RbxSync] Config not initialized with plugin reference"`
- `"No project set. Enter path above or pass project_dir in command."`
- `"Can't load API. Try reopening Studio."`
- `"Extraction failed. Try again."`
- `"[RbxSync] Sync error: {}"`
- `"[RbxSync] Failed to send response: {}"`
- `"Could not connect to server"`
- `"[RbxSync] Sync batch {}/{} failed"`
- `"[RbxSync] Sync sent {} changes but no files written - check project path"`
- `"[RbxSync] Cannot flush queue - not connected"`
- `"[RbxSync] Some flush batches failed"`
- `"[RbxSync] Failed to fetch API dump: {}"`
- `"[RbxSync] Failed to parse API dump: {}"`
- `"[RbxSync] No fallback API dump available"`
- `"[RbxSync] Failed to read terrain chunk at {}"`
- `"[RbxSync] Failed to set terrain color for {}: {}"`

### VS Code Extension Errors
- `"Not connected. Is Studio running?"`
- `"Open a folder first."`
- `"Failed to read files."`
- `"Sync failed. Try again."`
- `"Test failed: {}"`
- `"Failed to start server. Check the terminal for errors."`
- `"Failed to update Studio project path. Make sure Studio is connected."`
- `"No workspace folder open"`
- `"Failed to link studio"`
- `"Failed to unlink studio"`
- `"Failed to undo extraction"`
- `"Failed to undo extraction - no backup found"`
- `"Failed to recover: {}"`

</details>

---

*Report generated: 2026-01-16*
