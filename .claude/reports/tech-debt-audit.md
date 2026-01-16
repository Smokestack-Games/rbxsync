# Technical Debt Audit Report

**Date:** 2026-01-16
**Auditor:** Claude (automated)
**Scope:** rbxsync codebase (Rust, Luau)

---

## Executive Summary

The codebase is generally well-structured but has accumulated technical debt in several areas. The most critical issues are potential panics from `unwrap()` calls in production code paths and two very large files that need decomposition. Silent error handling via `let _ = ...` patterns is pervasive.

**Overall Health:** Moderate debt - needs attention before scaling

---

## 1. TODO/FIXME/HACK Comments

**Status:** Clean - no TODO/FIXME/HACK comments found in Rust or Luau code.

---

## 2. Unwrap/Expect Calls (Panic Risk)

### High Risk (Production Code Paths)

| File | Line | Code | Risk |
|------|------|------|------|
| `rbxsync-cli/src/main.rs` | 461, 1199, 1279, 1561, 1729, 2272, 2892 | `std::env::current_dir().unwrap()` | **High** - Panics if CWD unavailable |
| `rbxsync-cli/src/main.rs` | 357 | `.parse().unwrap()` on tracing directive | Medium - Should never fail |
| `rbxsync-cli/src/main.rs` | 2446, 2513, 2827, 2860 | `.to_str().unwrap()` on paths | **High** - Panics on non-UTF8 paths |
| `rbxsync-server/src/lib.rs` | 1045, 1070, 2021, 2090 | `serde_json::to_value(&x).unwrap()` | Medium - Unlikely to fail |
| `rbxsync-server/src/lib.rs` | 1363 | `serde_json::to_string_pretty(&output).unwrap()` | Medium - Unlikely to fail |
| `rbxsync-server/src/lib.rs` | 1419 | `session_guard.as_ref().unwrap()` | **High** - Panics if no session |
| `rbxsync-server/src/lib.rs` | 1480 | `std::fs::read_dir(".").unwrap()` as fallback | Medium - Defensive fallback |

### Low Risk (Test/Benchmark Code - Acceptable)

~50 `unwrap()` calls in:
- `benchmarks/src/benchmarks/*.rs`
- `benchmarks/benches/*.rs`
- `rbxsync-core/src/*/tests`

**Recommendation:** Replace high-risk `unwrap()` calls with proper error handling or `expect()` with descriptive messages.

---

## 3. Silent Error Handling (`let _ = ...`)

**Found:** 30+ instances of silently discarded results

### Categories

**File Operations (High Concern):**
```
rbxsync-server/src/lib.rs:937   let _ = std::fs::remove_dir_all(&backup_src);
rbxsync-server/src/lib.rs:1178  let _ = std::fs::remove_dir_all(&backup_src);
rbxsync-server/src/lib.rs:1182  let _ = std::fs::create_dir_all(&backup_dir);
rbxsync-server/src/lib.rs:1192  let _ = std::fs::remove_dir_all(&src_dir);
rbxsync-server/src/lib.rs:1199  let _ = std::fs::create_dir_all(&src_dir);
```

**Channel Operations (Acceptable):**
```
rbxsync-server/src/lib.rs:1087  let _ = sender.send(response);
rbxsync-server/src/lib.rs:1219  let _ = state.trigger.send(());
```

**Risk Assessment:**
- File deletion failures silently ignored = potential disk space issues
- Directory creation failures silently ignored = potential data loss
- Some are legitimate (fire-and-forget channels)

---

## 4. Duplicated Code Patterns

### File System Operations

**Pattern:** `std::fs::create_dir_all` called 40+ times across codebase
- Often with identical error handling (or lack thereof)
- Should be centralized into a helper function

**Examples:**
- `rbxsync-cli/src/main.rs`: 12 calls
- `rbxsync-server/src/lib.rs`: 18 calls

### File Reading Patterns

Repeated patterns for reading script files:
```rust
// Pattern appears ~15 times
if let Ok(content) = std::fs::read_to_string(&path) {
    // process content
}
```

**Recommendation:** Create utility functions:
- `ensure_dir_exists(path)` with logging
- `read_script_file(path) -> Result<ScriptContent>`

---

## 5. Large File Decomposition Needed

### rbxsync-server/src/lib.rs (4,056 lines)

**Risk:** High - difficult to maintain, test, and review

**Current Contents:**
- HTTP handlers (40+ endpoints)
- File watching logic
- Sync state management
- Bot control logic
- Git operations
- Extraction logic

**Recommended Split:**
| New Module | Responsibility | Estimated Lines |
|------------|----------------|-----------------|
| `handlers/mod.rs` | HTTP route handlers | ~800 |
| `handlers/extract.rs` | Extraction endpoints | ~500 |
| `handlers/sync.rs` | Sync endpoints | ~600 |
| `handlers/bot.rs` | Bot control endpoints | ~500 |
| `state.rs` | AppState and session management | ~300 |
| `operations/extract.rs` | Extraction business logic | ~600 |
| `operations/sync.rs` | Sync business logic | ~500 |

### rbxsync-cli/src/main.rs (2,994 lines)

**Risk:** Medium-High - single file CLI with many subcommands

**Recommended Split:**
| New Module | Commands |
|------------|----------|
| `commands/init.rs` | init, studio |
| `commands/serve.rs` | serve, stop, status |
| `commands/sync.rs` | sync, diff, build |
| `commands/debug.rs` | debug subcommands |
| `commands/update.rs` | update, version |

---

## 6. Dependency Concerns

### Version Mismatch

```toml
# Workspace (Cargo.toml)
reqwest = { version = "0.11", features = ["json"] }

# rbxsync-mcp (Cargo.toml)
reqwest = { version = "0.12", features = ["json"] }
```

**Risk:** Medium - different async runtime behavior between 0.11 and 0.12

**Recommendation:** Align all reqwest versions to 0.12

### Pinning Recommendations

Current deps use semver ranges. For reproducible builds, consider:
- Using `Cargo.lock` in version control (already done)
- Periodic dependency audits with `cargo audit`

---

## 7. Async Lock Patterns

**Found:** 18 instances of `.lock().await` on same mutex (`request_queue`)

```rust
// Pattern in rbxsync-server/src/lib.rs
state.request_queue.lock().await
```

**Concern:** Potential for lock contention under load

**Recommendation:** Consider:
- Using `tokio::sync::RwLock` where reads dominate
- Batch operations to reduce lock frequency
- Instrument with tracing to identify contention

---

## Priority Order for Fixes

### Quick Wins (1-2 hours each)

1. **Fix `current_dir().unwrap()` calls** - Replace with proper error handling
2. **Fix `to_str().unwrap()` on paths** - Use `to_string_lossy()` or error
3. **Align reqwest versions** - Update workspace to 0.12
4. **Add `expect()` messages** - Replace bare `unwrap()` with descriptive `expect()`

### Medium Effort (1-2 days each)

5. **Create file system utilities** - Centralize create_dir_all, read_to_string patterns
6. **Add logging to silent failures** - Replace `let _ =` with `if let Err(e) = ... { tracing::warn!() }`
7. **Split CLI into command modules** - Improve maintainability

### Major Refactors (1+ week each)

8. **Decompose `rbxsync-server/src/lib.rs`** - Split into handler and operation modules
9. **Review async lock patterns** - Profile and optimize mutex usage
10. **Add error context throughout** - Use `anyhow::Context` consistently

---

## Metrics Summary

| Metric | Count | Status |
|--------|-------|--------|
| TODO/FIXME/HACK | 0 | Clean |
| `unwrap()` in prod code | ~25 | Needs attention |
| `unwrap()` in test code | ~50 | Acceptable |
| Silent `let _ =` | 30+ | Needs review |
| Files >1000 lines | 2 | Needs decomposition |
| Dependency mismatches | 1 | Quick fix |

---

## Appendix: Full `unwrap()` Locations

### Production Code (needs fixing)

```
rbxsync-cli/src/main.rs:357
rbxsync-cli/src/main.rs:461
rbxsync-cli/src/main.rs:1199
rbxsync-cli/src/main.rs:1279
rbxsync-cli/src/main.rs:1561
rbxsync-cli/src/main.rs:1729
rbxsync-cli/src/main.rs:2272
rbxsync-cli/src/main.rs:2446
rbxsync-cli/src/main.rs:2513
rbxsync-cli/src/main.rs:2827
rbxsync-cli/src/main.rs:2860
rbxsync-cli/src/main.rs:2892
rbxsync-server/src/lib.rs:1045
rbxsync-server/src/lib.rs:1070
rbxsync-server/src/lib.rs:1363
rbxsync-server/src/lib.rs:1419
rbxsync-server/src/lib.rs:1480
rbxsync-server/src/lib.rs:2021
rbxsync-server/src/lib.rs:2090
```

---

*Report generated by technical debt audit task*
