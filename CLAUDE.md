# Claude Agent Instructions for RbxSync

> **Read this first.** This file provides context for AI agents working on rbxsync.

## What is RbxSync?

RbxSync is a bidirectional sync tool between Roblox Studio and local filesystem. It enables:
- Git-based version control for Roblox games
- External editor support (VS Code)
- AI-assisted development via MCP

**Current Version:** v1.3.0
**Status:** Active development

---

## Critical Context

### Current Priority: BUG BASH

**Recently fixed:**
- ~~RBXSYNC-24~~ - Data loss with ScriptSync
- ~~RBXSYNC-25~~ - Script timeout on large games
- ~~RBXSYNC-26~~ - Large game extraction slow
- ~~RBXSYNC-27~~ - Clear src folder before extraction
- ~~RBXSYNC-28~~ - Delete orphans UI in VS Code
- ~~RBXSYNC-30~~ - Extraction fails with excluded services
- ~~RBXSYNC-17~~ - Windows path corruption
- ~~RBXSYNC-33~~ - Zero-config mode
- ~~RBXSYNC-34~~ - Echo prevention flag
- ~~RBXSYNC-35~~ - 50ms deduplication window
- ~~RBXSYNC-36~~ - GetDebugId for instance IDs
- ~~RBXSYNC-38~~ - Union deletion during extract

**Active bugs:**
| Issue | Priority | Problem |
|-------|----------|---------|
| RBXSYNC-5 | - | Instance renames not handled |
| RBXSYNC-18 | - | Multiple terminal windows in VS Code |
| RBXSYNC-19 | - | Luau LSP can't find project.json |

---

## Project Structure

```
rbxsync/
├── rbxsync-core/     # Core serialization, DOM handling (Rust)
├── rbxsync-server/   # HTTP server, sync logic (Rust)
├── rbxsync-cli/      # CLI interface (Rust)
├── rbxsync-mcp/      # MCP server for AI tools (Rust)
├── rbxsync-vscode/   # VS Code extension (TypeScript)
├── plugin/           # Roblox Studio plugin (Luau)
└── .claude/          # AI agent configs and hooks
```

---

## Git Workflow

**Branch protection is enabled on `master`.** You must:

1. Create a feature branch:
   ```bash
   git checkout -b fix/rbxsync-XX-description
   ```

2. Make your changes and commit:
   ```bash
   git add .
   git commit -m "fix: description (Fixes RBXSYNC-XX)"
   ```

3. Push and create PR:
   ```bash
   git push -u origin fix/rbxsync-XX-description
   gh pr create --title "Fix: description" --body "Fixes RBXSYNC-XX"
   ```

**Never commit directly to master.**

---

## Linear Integration

All tasks are tracked in Linear (linear.app/smokestack-games).

- **Labels:** Bug, Feature, Improvement, Chore, Documentation + component labels (core, server, cli, mcp, vscode, plugin)
- **Projects:** Bug Bash, v1.2 Release, AI Integration, Org

When completing work, reference the issue: `Fixes RBXSYNC-XX`

### Linear MCP Tool Prefix

**ALWAYS use `mcp__linear-server__*` tools** (e.g., `mcp__linear-server__create_issue`).
**NEVER use `mcp__linear-audioscape__*`** — that is a different organization's workspace.

---

## Agent Teams

RbxSync uses Claude Code **Agent Teams** for multi-agent development. A team lead coordinates teammates who work in git worktrees.

### How It Works

1. **Team lead** creates an agent team and enables delegate mode
2. For each task, the lead creates a **git worktree** and spawns a **teammate** pointed at it
3. **Quality gate hooks** (`.claude/hooks/`) automatically enforce `cargo build`, `cargo test`, and `cargo clippy` before task completion
4. The lead **auto-merges PRs** after quality gates pass and updates Linear

### Teammate Instructions

If you are a teammate working on a task:

1. **Work in your assigned worktree** (path provided in your task)
2. Read relevant source files before modifying code
3. Commit with descriptive messages referencing the issue: `Fixes RBXSYNC-XX`
4. Push your branch and create a PR
5. **Mark your task complete** and message the lead with the PR URL
6. Quality gates will run automatically — fix any build/test/clippy failures before marking complete

### Branch Naming

| Type | Format | Example |
|------|--------|---------|
| Bug fix | `fix/rbxsync-XX-description` | `fix/rbxsync-71-terminal-reuse` |
| Feature | `feat/rbxsync-XX-description` | `feat/rbxsync-46-harness-tools` |
| Docs | `docs/rbxsync-XX-description` | `docs/rbxsync-63-mcp-reference` |
| Chore | `chore/rbxsync-XX-description` | `chore/rbxsync-67-warnings` |

---

## Before You Start

1. Read relevant files before modifying code
2. Create a branch for your work (or verify you're in an assigned worktree)

## After You Finish

1. Commit with descriptive message
2. Push and create PR if ready for review
3. Quality gates handle build/test/clippy validation automatically

---

## Key Files

| Component | Entry Point | Purpose |
|-----------|-------------|---------|
| Server | `rbxsync-server/src/server.rs` | HTTP server, sync logic |
| Core | `rbxsync-core/src/lib.rs` | DOM, serialization |
| MCP | `rbxsync-mcp/src/lib.rs` | AI tool handlers |
| Plugin | `plugin/src/Sync.luau` | Studio sync logic |
| VS Code | `rbxsync-vscode/src/extension.ts` | Extension entry |

---

## MCP Tools Available

When running with `rbxsync serve`, these MCP tools are available:

- `extract_game` - Extract game from Studio to files
- `sync_to_studio` - Push local changes to Studio
- `run_test` - Start playtest
- `run_code` - Execute Luau in Studio
- `bot_observe` - Get game state during playtest
- `bot_move` - Move character
- `bot_action` - Perform actions (equip, interact, etc.)

---

## Debug Logging & Test Verification

When writing features or fixing bugs, **always include structured debug output** so that test results can be verified automatically. Use the `TestLogger` module (`plugin/src/TestLogger.luau`) or follow the format manually.

### Log Format

All test output uses bracketed tags that are unambiguous for an LLM to parse:

```
[TEST BEGIN] suite_name
[TEST:test_name] PASS
[TEST:test_name] PASS — description
[TEST:test_name] FAIL — expected: X, actual: Y
[TEST:test_name] FAIL — reason
[SNAPSHOT:label] ClassName=Part, Name=Door, Position=10.00, 5.00, 0.00, ...
[EVENT:event_name] fired — args: arg1, arg2
[TEST SUMMARY] 3 passed, 1 failed, 4 total — FAIL
```

### Using TestLogger

The `TestLogger` module is bundled into the plugin. Require it in test scripts:

```lua
local TestLogger = require(game:GetService("CoreGui"):FindFirstChild("RbxSync").TestLogger)

TestLogger.begin("my_feature")

-- Assert equality
TestLogger.assert("health_value", 100, humanoid.Health)

-- Assert condition
TestLogger.assertTrue("part_anchored", part.Anchored)

-- Assert approximate numeric value
TestLogger.assertApprox("position_x", 10, part.Position.X, 0.5)

-- Assert non-nil
TestLogger.assertNotNil("module_exists", module)

-- Explicit pass/fail
TestLogger.pass("setup_complete", "All instances created")
TestLogger.fail("missing_remote", "RemoteEvent not found in ReplicatedStorage")

-- Snapshot instance state (before/after comparison)
TestLogger.snapshot("before_change", part)
part.Position = Vector3.new(10, 5, 0)
TestLogger.snapshot("after_change", part)

-- Log event verification
TestLogger.event("DoorOpened", {"TestPlayer"})

-- Print summary (returns true if all passed)
local allPassed = TestLogger.finish()
```

### Example Output

```
[TEST BEGIN] door_system
[TEST:door_exists] PASS
[TEST:door_starts_closed] PASS — transparency is 0
[SNAPSHOT:door_initial] ClassName=Part, Name=DoorPart, Position=10.00, 5.00, 0.00, Anchored=true, Transparency=0
[TEST:touch_connected] PASS
[EVENT:DoorOpened] fired — args: TestPlayer
[SNAPSHOT:door_after_open] ClassName=Part, Name=DoorPart, Position=10.00, 5.00, 0.00, Anchored=true, Transparency=1
[TEST:door_opened] PASS — door is transparent after opening
[TEST SUMMARY] 4 passed, 0 failed, 4 total — PASS
```

### When to Write Tests

- **Every feature**: Write at least one test that verifies the core behavior
- **Every bug fix**: Write a test that would have caught the bug
- **Before PR**: Run your tests via `run_test` and confirm `[TEST SUMMARY]` shows PASS
- **Property changes**: Use `snapshot()` before and after to show the diff

### Example Test Scripts

See `testing/examples/` for complete working examples:
- `sync-test.luau` — verifying synced instances exist with correct types
- `instance-properties-test.luau` — property assertions with before/after snapshots
- `event-test.luau` — event verification with argument checking

---

## Contact

- **Linear:** linear.app/smokestack-games
- **GitHub:** github.com/Smokestack-Games/rbxsync
- **Team Lead:** The main Claude session coordinating work via Agent Teams

---

*Last updated: 2026-02-13*
