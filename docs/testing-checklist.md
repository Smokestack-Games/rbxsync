# RbxSync Cross-Platform Testing Checklist

Use this checklist to manually verify RbxSync works correctly on Mac and Windows before releases.

## Prerequisites

- [ ] Roblox Studio installed and logged in
- [ ] RbxSync plugin installed in Studio
- [ ] VS Code with rbxsync extension (optional)
- [ ] A test place with: Scripts, LocalScripts, ModuleScripts, Parts, Models, Unions

---

## CLI Tests

### Server Startup
| Test | Mac | Windows | Notes |
|------|-----|---------|-------|
| `rbxsync serve` starts without errors | | | |
| Server binds to port 3000 | | | |
| Plugin connects successfully | | | |

### Extraction
| Test | Mac | Windows | Notes |
|------|-----|---------|-------|
| Extract small game (<100 instances) | | | |
| Extract medium game (100-1000 instances) | | | |
| Extract large game (1000+ instances) | | | |
| Scripts extracted as .luau files | | | |
| Parts/Models extracted as .rbxm files | | | |
| Correct folder structure created | | | |

### Path Handling
| Test | Mac | Windows | Notes |
|------|-----|---------|-------|
| Paths with spaces work | | | |
| Unicode characters in names | | | |
| Deep nested paths (>10 levels) | | | |
| No backslashes in output JSON | | | |

### Sync to Studio
| Test | Mac | Windows | Notes |
|------|-----|---------|-------|
| Edit script, sync reflects in Studio | | | |
| Create new file, appears in Studio | | | |
| Delete file, removed from Studio | | | |
| Rename file, renamed in Studio | | | |
| Large sync (>1MB) chunked correctly | | | |

### File Watcher
| Test | Mac | Windows | Notes |
|------|-----|---------|-------|
| Detects file changes | | | |
| No duplicate change events | | | |
| Handles rapid saves correctly | | | |

---

## VS Code Extension Tests

| Test | Mac | Windows | Notes |
|------|-----|---------|-------|
| Extension activates | | | |
| "Extract Game" command works | | | |
| "Sync to Studio" command works | | | |
| Terminal reuses existing window | | | |
| Status bar shows connection state | | | |
| project.json generated for LSP | | | |

---

## MCP Tools Tests (Claude Code)

| Test | Mac | Windows | Notes |
|------|-----|---------|-------|
| `extract_game` tool works | | | |
| `sync_to_studio` tool works | | | |
| `run_code` executes Luau | | | |
| `run_test` starts playtest | | | |
| `bot_observe` returns state | | | |
| `bot_move` moves character | | | |

---

## Edge Cases

| Test | Mac | Windows | Notes |
|------|-----|---------|-------|
| Empty place extraction | | | |
| Place with only terrain | | | |
| Scripts with syntax errors | | | |
| Binary data in scripts | | | |
| Very long instance names | | | |
| Special characters: `< > : " | ? *` | | | |

---

## Performance Benchmarks

Record approximate times:

| Operation | Mac | Windows |
|-----------|-----|---------|
| Extract 100 instances | | |
| Extract 1000 instances | | |
| Sync 10 files | | |
| Sync 100 files | | |

---

## Test Results

**Tester:** _______________
**Date:** _______________
**RbxSync Version:** _______________
**Mac Version:** _______________
**Windows Version:** _______________
**Studio Version:** _______________

### Issues Found
1.
2.
3.

### Notes

