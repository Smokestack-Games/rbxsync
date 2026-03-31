# RbxSync Handoff — Ben (vexedaa)

**Date:** 2026-03-30
**From:** Marissa Cheves (Smokestack Games)
**To:** Ben (vexedaa)

---

## What is RbxSync?

Bidirectional sync between Roblox Studio and local filesystem. Enables git-based version control, external editor support (VS Code extension), and AI-assisted game dev via MCP.

- **Current version:** v1.3.0
- **Status:** Paused (was active development, handing off now)
- **Repo:** https://github.com/Smokestack-Games/rbxsync
- **Docs site:** https://docs.rbxsync.dev
- **Marketing site:** https://rbxsync.dev
- **DevForum post:** https://devforum.roblox.com/t/4238545
- **Discord:** rbxsync.dev

---

## Architecture

```
rbxsync/
├── rbxsync-core/     # Core serialization, DOM handling (Rust)
├── rbxsync-server/   # HTTP server + sync logic (Rust, axum)
├── rbxsync-cli/      # CLI interface (Rust, clap)
├── rbxsync-mcp/      # MCP server for AI tools (Rust, rmcp)
├── rbxsync-vscode/   # VS Code extension (TypeScript)
├── plugin/           # Roblox Studio plugin (Luau)
├── docs/             # VitePress documentation site
├── website/          # Marketing site
├── testing/          # E2E test infrastructure
└── benchmarks/       # Performance benchmarks
```

**Port:** 44755 (default)

### Key entry points

| Component | File | Purpose |
|-----------|------|---------|
| Server | `rbxsync-server/src/server.rs` | HTTP server, sync logic |
| Core | `rbxsync-core/src/lib.rs` | DOM, serialization |
| MCP | `rbxsync-mcp/src/lib.rs` | AI tool handlers |
| Plugin | `plugin/src/Sync.luau` | Studio sync logic |
| VS Code | `rbxsync-vscode/src/extension.ts` | Extension entry |

---

## Build & Test

```bash
# Build everything
cargo build

# Run tests
cargo test

# Lint
cargo clippy

# Build VS Code extension
cd rbxsync-vscode && npm install && npm run compile

# Build plugin (.rbxm)
cargo run -- build-plugin
```

---

## Releasing

See `RELEASING.md` for full details. TL;DR:
1. Tag: `git tag v1.X.X && git push origin v1.X.X`
2. GitHub Actions builds CLI + plugin + creates release
3. Deploy docs/website manually: `cd website && vercel --prod`

---

## Open Bugs (Priority Order)

| Issue | Priority | Problem |
|-------|----------|---------|
| RBXSYNC-38 | **P1 Urgent** | Union deletion during extract |
| RBXSYNC-5 | P2 | Instance renames not handled |
| RBXSYNC-18 | P3 | Multiple terminal windows in VS Code |
| RBXSYNC-19 | P3 | Luau LSP can't find project.json |
| [#123](https://github.com/Smokestack-Games/rbxsync/issues/123) | Bug | RbxSync Link not functional |
| [#121](https://github.com/Smokestack-Games/rbxsync/issues/121) | Bug | Chunks being stuck/overloading |

---

## Backlog (Features & Improvements)

### From internal tracking (RBXSYNC-* are Linear issue IDs)

- [ ] RBXSYNC-119: `run_test` playtest ends unexpectedly on some places
- [ ] RBXSYNC-96: `run_test` missing `background` parameter in MCP schema
- [ ] RBXSYNC-94: Stale session spam during playtest
- [ ] RBXSYNC-98: `run_test` reliability and `test_status` tool
- [ ] RBXSYNC-90: Multiple CLI installations cause version conflicts
- [ ] RBXSYNC-111: Multi-place game support (required by Wonder Works Studio)
- [ ] RBXSYNC-116: AI agent prompt/guide for creating Roblox objects
- [ ] RBXSYNC-88: Formalize build artifact paths and cleanup
- [ ] Prepare Wonder Works Studio onboarding materials

### From GitHub Issues

| # | Title | Type |
|---|-------|------|
| [#169](https://github.com/Smokestack-Games/rbxsync/issues/169) | AI Assistant Bridge: System Prompts and Remote CLI Querying | Feature |
| [#168](https://github.com/Smokestack-Games/rbxsync/issues/168) | Suppress plugin log output (--quiet / --log-level) | Feature |
| [#167](https://github.com/Smokestack-Games/rbxsync/issues/167) | MCP server binary for Windows x64 in releases | Feature |
| [#135](https://github.com/Smokestack-Games/rbxsync/issues/135) | CONTRIBUTING.md references rojo instead of rbxsync | Docs |
| [#134](https://github.com/Smokestack-Games/rbxsync/issues/134) | MCP: Studio selection and class reflection tools | Feature |
| [#133](https://github.com/Smokestack-Games/rbxsync/issues/133) | MCP: CollectionService tag management tools | Feature |
| [#132](https://github.com/Smokestack-Games/rbxsync/issues/132) | MCP: Attribute management tools | Feature |
| [#131](https://github.com/Smokestack-Games/rbxsync/issues/131) | MCP: Script source editing tools | Feature |
| [#130](https://github.com/Smokestack-Games/rbxsync/issues/130) | MCP: Instance creation and deletion tools | Feature |
| [#129](https://github.com/Smokestack-Games/rbxsync/issues/129) | MCP: Direct property manipulation tools | Feature |
| [#124](https://github.com/Smokestack-Games/rbxsync/issues/124) | Self Hosting | Feature |
| [#109](https://github.com/Smokestack-Games/rbxsync/issues/109) | Linux binary | Feature |

---

## MCP Tools (AI Integration)

When running `rbxsync serve`, these MCP tools are available:

| Tool | Purpose |
|------|---------|
| `extract_game` | Extract game from Studio to files |
| `sync_to_studio` | Push local changes to Studio |
| `run_test` | Start playtest |
| `run_code` | Execute Luau in Studio |
| `bot_observe` | Get game state during playtest |
| `bot_move` | Move character |
| `bot_action` | Perform actions (equip, interact, etc.) |

Issues #129-134 are all about expanding this MCP tool surface.

---

## Git Workflow

- **Branch protection on `master`** — never commit directly
- Branch naming: `fix/rbxsync-XX-desc`, `feat/`, `docs/`, `chore/`
- Reference issues in PRs: `Fixes RBXSYNC-XX` or `Fixes #XX`
- Squash merge preferred

---

## Linear (Legacy)

We used Linear (linear.app/smokestack-games) for issue tracking earlier. The RBXSYNC-* IDs reference Linear issues. Linear is no longer actively maintained — **use GitHub Issues going forward**. The `.claude/MANAGER-GUIDE.md` has Linear project/cycle IDs if you ever need to reference old context.

---

## Competitors

- **Rojo** — The established Roblox sync tool. RbxSync aims to be easier to set up and includes AI/MCP features Rojo doesn't have.
- **Pesto** — Free/open-source VS Code ↔ Studio sync. Appeared on DevForum Dec 2025.

---

## Business Context

- **Pricing:** Flat $60/license via Stripe
- **Beta users:** 30+, 15 GitHub stars
- **First customer pipeline:** Wonder Works Studio (needs multi-place support, RBXSYNC-111)
- **Metrics to watch:** GitHub stars, Discord members, beta signups

---

## Agent Workflow (for Claude Code users)

The `.claude/` directory has everything for AI-assisted dev:
- `CLAUDE.md` — Agent instructions (read this first)
- `.claude/MANAGER-GUIDE.md` — How to coordinate multi-agent work
- `.claude/templates/tps-report.md` — Completion report template
- `.claude/reports/` — Past worker reports (good for understanding what was done)
- `.claude/state/workers.json` — Worker tracking

Standard pattern: create worktree → work on branch → PR → merge → cleanup.

---

## Questions?

Reach out to Marissa: marissacheves@gmail.com
