# VS Code Commands

Access commands via `Cmd+Shift+P` (Mac) or `Ctrl+Shift+P` (Windows).

## Connection Commands

### RbxSync: Connect to Studio
Connect to the RbxSync server.

### RbxSync: Disconnect
Disconnect from the server.

## Sync Commands

### RbxSync: Extract Game from Studio
Pull the entire game from Studio to local files.

### RbxSync: Sync Changes to Studio
Push local file changes to Studio.

## Console Commands

### RbxSync: Open Console
Open a terminal showing Studio console output.

All `print()`, `warn()`, and `error()` from Studio appear in real-time:
- White: Info messages
- Yellow: Warnings
- Red: Errors

### RbxSync: Clear Console
Clear the console output.

## E2E Testing Commands

### RbxSync: Toggle E2E Mode
Enable or disable E2E testing mode.

When enabled, the console terminal auto-opens during operations.

### RbxSync: Run Playtest
Start a playtest in Studio from VS Code.

### RbxSync: Stop Playtest
Stop the current playtest.

## Git Commands

### RbxSync: Show Git Status
Display git repository status.

### RbxSync: Commit Changes
Commit staged changes with a message.

## Keyboard Shortcuts

| Shortcut | Command |
|----------|---------|
| `Cmd+Shift+E` | Extract Game |
| `Cmd+Shift+S` | Sync Changes |
| `Cmd+Shift+C` | Open Console |

## Related

- [Luau LSP Integration](/vscode/luau-lsp) - Intellisense and type checking setup
- [E2E Testing](/vscode/e2e-testing) - Automated testing workflows
