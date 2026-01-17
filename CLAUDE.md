# Claude Agent Instructions for RbxSync

> **Read this first.** This file provides context for AI agents working on rbxsync.

## WORKER AGENTS: DO NOT RUN RALPH LOOP

If you are a worker agent spawned to complete a specific task:
- **DO NOT** run `/ralph-loop` or any loop commands
- **DO NOT** read or act on `.claude/ralph-loop.local.md`
- **ONLY** complete your assigned issue, create PR, write report, then exit
- The manager agent handles the ralph loop, not workers

## What is RbxSync?

RbxSync is a bidirectional sync tool between Roblox Studio and local filesystem. It enables:
- Git-based version control for Roblox games
- External editor support (VS Code)
- AI-assisted development via MCP

**Current Version:** v1.3.0
**Status:** Active development, some critical bugs

---

## Critical Context

### Current Priority: BUG BASH

**Recently fixed (this session):**
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

**Active bugs:**
| Issue | Priority | Problem |
|-------|----------|---------|
| RBXSYNC-38 | P1 Urgent | Union deletion during extract |
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
└── .claude/          # AI agent configs and state
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

All tasks are tracked in Linear (linear.app/rbxsync).

- **Labels:** Bug, Feature, Improvement, Chore, Documentation + component labels (core, server, cli, mcp, vscode, plugin)
- **Projects:** Bug Bash, v1.2 Release, AI Integration, Org

When completing work, reference the issue: `Fixes RBXSYNC-XX`

---

## CRITICAL: Multi-Agent Coordination

Multiple Claude agents work on this project simultaneously. **You MUST follow these rules to avoid conflicts.**

### Before Starting ANY Work

1. **Check `.claude/state/workers.json`** - See who's working on what
2. **Check Linear issue status** - If it's "In Progress", someone else has it
3. **If the issue is taken**, pick a different one or ask the manager

### When You Start Work

1. **Update Linear immediately:**
   - Move issue to "In Progress"
   - Add a comment: "Agent starting work on this issue"

2. **Use Linear's branch name:**
   ```bash
   # Get branch name from Linear issue
   git checkout -b marissacheves/rbxsync-XX-description
   ```

3. **Update `.claude/state/workers.json`** if you can (manager will help)

### While Working

1. **Add Linear comments for major progress:**
   - "Found the bug in server.rs:450"
   - "Implementing fix, testing now"
   - "Tests passing, preparing PR"

2. **If you get stuck**, add comment: "Blocked: [reason]"

### When You Finish

1. **Create PR** with `Fixes RBXSYNC-XX` in description
2. **Update Linear:**
   - Move to "In Review" or "Done"
   - Add comment with summary of changes
3. **Write completion report** to `.claude/reports/worker-RBXSYNC-XX.md`:
   ```markdown
   # Worker Report: RBXSYNC-XX
   **Date:** [timestamp]
   **Status:** Complete | Blocked | Partial

   ## Summary
   [1-2 sentence summary]

   ## Changes Made
   - [file]: [what changed]

   ## PR
   - Number: #XX
   - Branch: fix/rbxsync-XX-description

   ## Issues Encountered
   [any blockers or surprises]

   ## Notes for Manager
   [anything the manager should know]
   ```
4. **Print completion signal:** `DONE: RBXSYNC-XX - [summary]`

### Conflict Resolution

If you discover another agent is working on your issue:
1. **STOP immediately**
2. **Do NOT commit**
3. **Report the conflict** to manager
4. **Pick a different issue**

---

## Before You Start

1. Run `cargo build` to verify the project compiles
2. Check `git status` for any uncommitted changes
3. Create a branch for your work
4. Read relevant files before modifying

## After You Finish

1. Run `cargo build` - fix any errors
2. Run `cargo test` - fix any failures
3. Run `cargo clippy` - fix any warnings
4. Commit with descriptive message
5. Create PR if ready for review

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

## Contact

- **Linear:** linear.app/rbxsync
- **GitHub:** github.com/devmarissa/rbxsync
- **Manager Agent:** The main Claude session coordinating work

---

*Last updated: 2026-01-16*
