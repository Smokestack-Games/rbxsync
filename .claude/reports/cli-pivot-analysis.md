# CLI-Only Pivot Analysis

**Date:** 2026-01-16
**Status:** Research Complete

---

## Executive Summary

RbxSync currently has three main components: CLI, VS Code extension, and Studio plugin. Given that the primary users (AI tools like Claude Code and Antigravity) operate through MCP, not VS Code, there's a case for simplifying to CLI + MCP only. This analysis examines what would be gained and lost.

**Recommendation:** Partial pivot - deprecate VS Code extension UI, but extract the LSP as a standalone tool that works with any editor.

---

## 1. Current Feature Matrix

| Feature | CLI | VS Code | Studio Plugin | MCP |
|---------|-----|---------|---------------|-----|
| **Core Sync** |
| Extract game to files | `rbxsync extract` | Button | Button | `extract_game` |
| Sync files to Studio | `rbxsync sync` | Button | Button | `sync_to_studio` |
| Server management | `rbxsync serve/stop` | Start/Stop button | Connect/Disconnect | (uses server) |
| **Git Integration** |
| Git status | - | - | - | `git_status` |
| Git commit | - | - | - | `git_commit` |
| **Playtest** |
| Run playtest | `rbxsync debug start` | Test button | - | `run_test` |
| Console capture | - | Terminal streaming | E2E mode | `run_test` output |
| Bot control (AI gameplay) | - | - | - | `bot_*` tools (6) |
| **Build** |
| Build .rbxl/.rbxm | `rbxsync build` | - | - | - |
| Build plugin | `rbxsync build-plugin` | - | - | - |
| Sourcemap generation | `rbxsync sourcemap` | - | - | - |
| **DX Features** |
| .rbxjson LSP | - | Yes (completion, hover, diagnostics) | - | - |
| Status bar | - | Yes | - | - |
| Sidebar UI | - | Yes (zen cat, operations, toggles) | - | - |
| Trash recovery | - | Yes | - | - |
| Auto-connect | - | Yes | - | - |
| **Code Execution** |
| Run Luau in edit mode | - | - | HTTP API | `run_code` |
| Query server during playtest | - | - | - | `bot_query_server` |

---

## 2. What AI Tools Actually Use

Based on the MCP tool definitions and AI workflow patterns:

### Primary Usage (Daily)
- `extract_game` - Pull game state to files
- `sync_to_studio` - Push code changes
- `run_code` - Execute Luau for quick tests/queries
- `run_test` - Automated playtesting with console capture
- Standard file operations (Read, Write, Edit via Claude tools)

### Bot Controller (Gameplay Testing)
- `bot_observe` - Get game state (position, health, inventory, nearby)
- `bot_move` - Pathfinding navigation
- `bot_action` - Equip, interact, jump
- `bot_query_server` - Execute Luau on server during playtest
- `bot_wait_for` - Wait for conditions

### Git Integration
- `git_status` - Check changes
- `git_commit` - Commit (though AI agents often use Bash git directly)

### NOT Used by AI
- VS Code sidebar UI
- Status bar indicators
- Zen cat mascot (Sink)
- Click-based sync/extract buttons
- E2E mode toggle
- .rbxjson LSP features
- Trash recovery

---

## 3. What Would Be Lost in CLI-Only

### For AI Workflows: Nothing Significant
AI agents interact exclusively through MCP. The VS Code extension provides zero functionality that MCP doesn't already expose.

### For Human Developers

| Lost Feature | Impact | Mitigation |
|--------------|--------|------------|
| Sidebar UI | High for VS Code users | Terminal-based TUI or web dashboard |
| .rbxjson LSP | Medium - nice autocomplete | Extract as standalone LSP, integrate with Luau LSP |
| Status bar | Low - cosmetic | CLI status command |
| Click-to-sync | Medium for mouse users | Keyboard shortcuts, CLI aliases |
| Auto-connect | Low | `rbxsync serve` in terminal profile |
| Trash recovery | Low | Add `rbxsync undo` command |
| Zen cat | Zero functional impact | Keep in docs as mascot |

### Unique VS Code Features Worth Preserving
1. **LSP for .rbxjson** - The completion, hover, and diagnostics for metadata files is genuinely useful
2. **Console streaming** - Already available via MCP's `run_test`

---

## 4. Recommendation: Partial Pivot

### Do Deprecate
- **VS Code sidebar webview** - Heavy maintenance burden (2100+ lines), no AI value
- **Status bar** - Redundant with CLI status
- **E2E mode toggle** - MCP handles this natively
- **Zen cat UI** - Fun but non-functional

### Do Keep (As Standalone)
- **LSP server** - Extract as `rbxsync-lsp` that any editor can use
- Luau LSP could potentially consume this via plugin architecture

### Do Add to CLI
- `rbxsync status --watch` - Live terminal status
- `rbxsync undo` - Recover from trash
- Better terminal output formatting for human readability

### Tradeoffs

| Pros | Cons |
|------|------|
| Simpler architecture | VS Code users lose GUI |
| Faster iteration (no TS build) | Onboarding slightly harder |
| Smaller install footprint | No click-based workflow |
| Single source of truth (CLI) | Need to document CLI workflow |
| AI-first design | - |

---

## 5. Migration Path (If Pivoting)

### Phase 1: Extract LSP (1 week)
1. Move `rbxsync-vscode/src/lsp/` to `rbxsync-lsp/`
2. Create standalone binary/npm package
3. Update docs for editor-agnostic setup
4. Test with VS Code, Neovim, Helix

### Phase 2: Deprecation Notice (2 weeks)
1. Add deprecation banner to VS Code extension
2. Create migration guide: VS Code -> CLI workflow
3. Add CLI aliases for common operations

### Phase 3: Sunset Extension (1 month)
1. Stop publishing new versions to marketplace
2. Archive `rbxsync-vscode/` directory
3. Update docs to CLI-only

### Phase 4: CLI Improvements (Ongoing)
1. Add `rbxsync tui` for interactive terminal UI (optional)
2. Web dashboard option (localhost:44755/ui)
3. Better formatted output with colors/spinners

---

## 6. User Segment Analysis

| User Type | Current Tool | After Pivot | Impact |
|-----------|--------------|-------------|--------|
| AI (Claude Code) | MCP | MCP | None |
| AI (Antigravity) | MCP | MCP | None |
| Human (VS Code) | Extension | CLI + LSP | Medium |
| Human (Neovim/other) | CLI | CLI + LSP | Improved |
| Teams | Mixed | CLI + Git | Simplified |

---

## Appendix: VS Code Extension Complexity

From `rbxsync-vscode/src/views/sidebarWebview.ts`:
- 2100+ lines of inline HTML/CSS/JS
- Zen cat ASCII art and animations
- Custom design system
- Complex state management
- Multiple message handlers

This represents significant maintenance burden for features AI agents don't use.

---

## Conclusion

The VS Code extension was valuable in the pre-AI era when humans clicked buttons. Now that AI agents drive most workflows through MCP, the extension has become maintenance overhead. A CLI-only approach with a standalone LSP would:

1. Reduce complexity
2. Improve AI workflow (already optimized for CLI)
3. Support all editors equally
4. Focus engineering effort on core value (sync + git)

The main risk is alienating human VS Code users, but this can be mitigated by:
- Good CLI UX
- Optional TUI or web dashboard
- Standalone LSP that works everywhere
