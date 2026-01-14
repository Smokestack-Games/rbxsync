# RbxSync v1.1.0 - Wally Support, Multi-Studio & Major Bug Fixes

This is a significant release with new features and critical bug fixes that improve reliability across the board.

## New Features

### Wally Package Manager Support
RbxSync now fully supports Wally packages! Configure your package directories in `rbxsync.json` and they'll be:
- **Preserved during extraction** - Your installed packages won't be overwritten
- **Excluded from file watching** - No accidental syncs of package code
- **Detected automatically** - Reads `wally.toml` and `wally.lock` for smart handling

### Multi-Studio Support
Connect multiple Roblox Studio instances simultaneously! Each Studio session now has a unique ID, allowing you to work on multiple places at once without conflicts.

### Redesigned VS Code Sidebar
The sidebar has been completely rebuilt with a custom webview that shows:
- All connected Studio instances as cards
- Per-studio Sync, Extract, and Test buttons
- Server status and controls
- Toast notifications for operation results

### Terrain Support
Full terrain extraction and synchronization is now supported.

### CSG/Union Support
UnionOperations can now be extracted and reconstructed using CSG APIs.

### Diff Command
New `rbxsync diff` command to compare local files against what's in Studio.

---

## Bug Fixes

### Critical Fixes
- **Parts Being Overwritten** - Fixed a major bug where Parts would be incorrectly overwritten during sync. Now uses `_rbxsync_source` attribute for exact path matching.
- **Multi-Studio Conflicts** - Extraction in one Studio no longer triggers sync commands in other connected Studios.
- **MeshPart Sync** - Fixed MeshPart synchronization to use `CreateMeshPartAsync` correctly.

### Other Fixes
- Fixed MaterialService sync API calls
- Fixed terrain.rbxjson filtering
- Fixed instance reference extraction and sync
- Fixed class search dropdown layout
- Improved rbxjson LSP completions and diagnostics

---

## Installation

**CLI**: See install instructions on GitHub releases page

**VS Code Extension**: Search "RbxSync" in the Extensions marketplace

**Roblox Plugin**: Install from Creator Store (auto-updates)

---

## Quick Links

| Resource | Link |
|----------|------|
| GitHub | github.com/devmarissa/rbxsync |
| Documentation | docs.rbxsync.dev |

---

Thank you for using RbxSync! If you encounter any issues, please report them on GitHub.
