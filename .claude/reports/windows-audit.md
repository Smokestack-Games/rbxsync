# Windows Compatibility Audit Report

**Date:** 2026-01-16
**Auditor:** Claude (Worker Agent)
**Scope:** Full codebase audit for Windows compatibility

---

## Executive Summary

The codebase has **partial Windows support** with some cross-platform patterns in place, but several areas require attention. The core sync functionality should work on Windows, but CLI utilities for process management and some helper features are Unix-only.

**Risk Level: MEDIUM** - Core functionality works, but several edge cases and utilities will fail on Windows.

---

## Issues Found

### 1. Hardcoded Unix Paths

| File | Line | Issue | Risk |
|------|------|-------|------|
| `rbxsync-cli/src/main.rs` | 304-306 | Hardcoded `/usr/local/bin/rbxsync`, `/usr/bin/rbxsync`, `~/.cargo/bin/rbxsync` for binary detection | **MEDIUM** |
| `rbxsync-cli/src/main.rs` | 741 | `PathBuf::from("/Applications/RobloxStudio.app")` | LOW (cfg-guarded) |
| `rbxsync-cli/src/imessage.rs` | 36 | `~/Library/Messages/chat.db` | LOW (macOS-only feature) |
| `scripts/install.sh` | 67 | `INSTALL_DIR="/usr/local/bin"` | LOW (bash script) |

**Recommended Fix:** Add Windows-equivalent paths for binary detection:
```rust
#[cfg(windows)]
vec![
    format!("{}\\rbxsync\\rbxsync.exe", std::env::var("LOCALAPPDATA").unwrap_or_default()),
    format!("{}\\rbxsync.exe", std::env::var("USERPROFILE").unwrap_or_default()),
]
```

---

### 2. Unix-Specific Process Management

| File | Line | Issue | Risk |
|------|------|-------|------|
| `rbxsync-cli/src/main.rs` | 1020-1047 | `stop_all_servers()` uses `pgrep` - Unix only | **HIGH** |
| `rbxsync-cli/src/main.rs` | 1084 | `lsof` command for port detection | **HIGH** |
| `rbxsync-cli/src/main.rs` | 1039, 1116, 1132 | `libc::kill(pid, SIGTERM/SIGKILL)` | **HIGH** |
| `rbxsync-cli/src/main.rs` | 972 | `std::os::unix::process::CommandExt` for exec() | **MEDIUM** |

**Current Mitigation:** There's a fallback for non-Unix:
```rust
#[cfg(not(unix))]
async fn stop_all_servers() -> Result<()> {
    println!("Stopping all servers is only supported on Unix systems.");
    // ...
}
```

**Recommended Fix:** Implement Windows equivalents:
- Use `tasklist` and `taskkill` for process management
- Use `netstat -an | findstr PORT` for port detection
- Use Windows `TerminateProcess` API instead of signals

---

### 3. Path Separator Assumptions

| File | Line | Issue | Risk |
|------|------|-------|------|
| `rbxsync-vscode/src/views/sidebarWebview.ts` | 2069 | `p.split('/')` - assumes forward slashes | **MEDIUM** |
| `rbxsync-vscode/src/views/activityView.ts` | 369 | `fullPath.split('/').filter(Boolean)` | **MEDIUM** |
| `rbxsync-vscode/src/commands/sync.ts` | 76 | `studioPath.split('/')` | **MEDIUM** |
| `rbxsync-server/src/lib.rs` | 1636 | `fs_path.split('/').next()` | LOW (internal paths) |
| `rbxsync-core/src/rojo.rs` | 198 | `normalized.split('/').next()` | LOW (normalized paths) |

**Mitigating Factor:** The codebase has `path_utils.rs` with `normalize_path()` that converts backslashes to forward slashes. Internal instance paths always use forward slashes.

**Recommended Fix:** For TypeScript files:
```typescript
const parts = p.replace(/\\/g, '/').split('/');
```

---

### 4. macOS-Only Features

| File | Feature | Issue | Risk |
|------|---------|-------|------|
| `rbxsync-cli/src/imessage.rs` | iMessage integration | Entire module is macOS-only (AppleScript, Messages.db) | LOW |
| `rbxsync-cli/src/main.rs` | 591 | `Command::new("open")` for launching apps | LOW (cfg-guarded) |

**Status:** These are properly guarded with `#[cfg(target_os = "macos")]`. No action needed.

---

### 5. Home Directory Handling

| File | Line | Pattern | Status |
|------|------|---------|--------|
| `rbxsync-vscode/src/views/statusBar.ts` | 102 | `process.env.HOME \|\| process.env.USERPROFILE` | **OK** |
| `rbxsync-vscode/src/views/activityView.ts` | 364 | `process.env.HOME \|\| process.env.USERPROFILE` | **OK** |
| `rbxsync-vscode/src/lsp/apiDump.ts` | 108, 133 | `process.env.HOME \|\| process.env.USERPROFILE` | **OK** |
| `rbxsync-core/src/plugin_builder.rs` | 180, 199 | `dirs::home_dir()` | **OK** |
| `rbxsync-cli/src/main.rs` | 747 | `std::env::var("HOME")` | **NEEDS FIX** |
| `rbxsync-cli/src/main.rs` | 306 | `std::env::var("HOME")` | **NEEDS FIX** |
| `rbxsync-cli/src/imessage.rs` | 35 | `std::env::var("HOME")` | OK (macOS-only) |

**Recommended Fix:** Replace raw `HOME` access with `dirs::home_dir()`:
```rust
dirs::home_dir()
    .map(|h| h.join(".cargo/bin/rbxsync"))
    .filter(|p| p.exists())
```

---

### 6. File Permissions

| File | Line | Issue | Risk |
|------|------|-------|------|
| `scripts/install.sh` | 113 | `chmod +x` | LOW (bash script) |
| Documentation | Various | `chmod +x rbxsync` instructions | LOW (docs) |

**Status:** These are in Unix shell scripts and documentation, which is expected. Windows doesn't need execute permissions.

---

## Good Patterns Already Present

The codebase has several good cross-platform patterns:

1. **`rbxsync-core/src/path_utils.rs`** - Normalizes paths to forward slashes
2. **`sanitize_filename()`** - Removes Windows-invalid characters (`<>:"|?*`)
3. **Platform-specific code blocks** - Proper use of `#[cfg(target_os)]`
4. **Windows Studio detection** - `find_studio_path()` checks LOCALAPPDATA
5. **VS Code extension** - Uses `HOME || USERPROFILE` pattern

---

## Risk Summary

| Category | Count | Risk Level |
|----------|-------|------------|
| Critical (blocks basic functionality) | 0 | - |
| High (breaks common operations) | 3 | Process management |
| Medium (breaks some features) | 5 | Path handling, home dir |
| Low (edge cases or documented) | 6 | macOS features, docs |

---

## Recommendations

### Priority 1 (High)
1. **Implement Windows process management** in `stop_server_on_port()` and `stop_all_servers()`
   - Use `tasklist` / `taskkill` or Windows API
   - Use `netstat` for port detection

### Priority 2 (Medium)
2. **Replace raw HOME env var** with `dirs::home_dir()` in main.rs
3. **Add path normalization** in VS Code TypeScript files before splitting

### Priority 3 (Low)
4. **Document Windows limitations** for iMessage feature
5. **Add Windows install script** (PowerShell)

---

## Test Recommendations

Before releasing for Windows, test:
- [ ] `rbxsync serve` - start server
- [ ] `rbxsync stop` - stop server (currently limited)
- [ ] `rbxsync extract` - extract game
- [ ] `rbxsync sync` - sync to Studio
- [ ] VS Code extension - full functionality
- [ ] Path handling with spaces and special characters

---

*Report generated by Claude Code audit*
