# Plugin Security & Quality Review

**Date:** 2026-01-16
**Reviewer:** Claude (automated review)
**Scope:** `plugin/` directory - Roblox Studio plugin for RbxSync

---

## Executive Summary

The RbxSync Studio plugin is a moderately complex codebase (~4500 lines) handling bidirectional sync between Studio and the filesystem. Overall code quality is good with proper type annotations (`--!strict`) and reasonable separation of concerns. However, there are several areas requiring attention around memory management, race conditions, and error handling.

**Critical Issues:** 0
**High Severity:** 2
**Medium Severity:** 5
**Low Severity:** 6
**Informational:** 3

---

## 1. Security Issues

### HIGH: Unbounded pending changes accumulation (RBXSYNC-SEC-001)
**File:** `ChangeTracker.luau:48-55`
**Severity:** High

The `pendingChanges` table grows unbounded when the server is unreachable. While `MAX_BATCH_SIZE` limits what gets sent, there's no limit on accumulation.

```lua
local pendingChanges: {[string]: {...}} = {}
-- No size limit check before adding
```

**Impact:** Memory exhaustion if server is down for extended periods during active editing.

**Recommendation:** Add a maximum pending changes limit (e.g., 10,000 entries) and drop oldest changes when exceeded.

---

### HIGH: Event connection leak on service tracking (RBXSYNC-SEC-002)
**File:** `ChangeTracker.luau:42, 555-592`
**Severity:** High

The `connections` array holds event connections for every tracked instance. For large games (100k+ instances), this creates thousands of active connections. These are only cleared when `stop()` is called.

```lua
local connections: {RBXScriptConnection} = {}
-- Each instance gets 2 connections (Changed + Name changed)
-- For 100k instances = 200k connections
```

**Impact:** Memory leak and potential performance degradation in long sessions.

**Recommendation:** Consider tracking only scripts/critical instances, or implement selective tracking based on user configuration.

---

### MEDIUM: No input validation on server URL (RBXSYNC-SEC-003)
**File:** `Config.luau:107-111`
**Severity:** Medium

The server URL is stored directly from user input without validation.

```lua
function Config.setServerUrl(url: string)
    local settings = Config.load()
    settings.serverUrl = url  -- No validation
    Config.save(settings)
end
```

**Impact:** User could be tricked into pointing to a malicious server that captures game data.

**Recommendation:** Validate URL format and restrict to localhost/127.0.0.1 by default, requiring explicit user confirmation for other hosts.

---

### MEDIUM: HTTP without TLS (RBXSYNC-SEC-004)
**File:** `Config.luau:21`
**Severity:** Medium

Default server URL uses unencrypted HTTP:

```lua
serverUrl = "http://localhost:44755",
```

**Impact:** While localhost traffic is typically safe, if the server binds to 0.0.0.0, traffic could be intercepted on the local network.

**Recommendation:** Consider HTTPS support for non-localhost connections.

---

### LOW: Silent credential exposure risk (RBXSYNC-SEC-005)
**File:** `Serializer.luau:432-548`
**Severity:** Low

Instance attributes are serialized without filtering. If a user stores sensitive data in attributes (API keys, tokens), they would be extracted to files.

**Recommendation:** Add configurable attribute filtering or warn users about attribute extraction.

---

### LOW: Code execution feature disabled but documented (RBXSYNC-SEC-006)
**File:** `init.server.luau:1504-1506`
**Severity:** Low (Informational)

The `run:code` command is disabled for Creator Store version - this is a good security decision.

```lua
elseif command == "run:code" then
    return { success = false, error = "run:code disabled in Creator Store version." }
```

**Status:** Properly mitigated.

---

## 2. Performance Bottlenecks

### Event connection overhead (PERF-001)
**File:** `ChangeTracker.luau:420-552`
**Severity:** Medium

Every tracked instance gets two event connections (`Changed` and `GetPropertyChangedSignal("Name")`). For a game with 50,000 instances, that's 100,000 active event connections.

**Impact:**
- High memory usage for connection objects
- Event dispatch overhead
- Slower instance operations

**Recommendation:**
- Track only script source changes (most common sync need)
- Use `DescendantAdded`/`DescendantRemoving` on services instead of individual connections
- Implement lazy connection - only track instances that have been modified

---

### Serialization without caching (PERF-002)
**File:** `Serializer.luau:432-548`
**Severity:** Low

Each extraction re-serializes all properties via reflection without caching property lists.

```lua
for _, propInfo in properties do
    if Reflection.shouldSerializeProperty(propInfo) then
        -- pcall for every property read
```

**Recommendation:** Cache the filtered property list per class to avoid repeated `shouldSerializeProperty` checks.

---

### Polling loop frequency (PERF-003)
**File:** `init.server.luau:1595-1708`
**Severity:** Low

The main poll loop runs every 1 second with HTTP requests. Combined with the change tracker's 0.5-second interval, this creates constant background activity.

**Recommendation:** Consider adaptive polling - increase interval when idle, decrease during active sync.

---

## 3. Memory Leaks / Unbounded Growth

### Instance caches without bounds (MEM-001)
**File:** `Sync.luau:22-23, 293-296`
**Severity:** Medium

Multiple caches grow throughout the session without bounds or weak references:

```lua
local instanceCache: {[string]: Instance} = {}
local instanceByRefId: {[string]: Instance} = {}
local recentlyCreated: {[string]: Instance} = {}
```

**Impact:** Memory grows linearly with sync operations.

**Recommendation:**
- Use weak value tables for instance caches
- Clear caches periodically or on threshold
- `Sync.clearCache()` exists but is only called on explicit sync operations

---

### Path tracking table (MEM-002)
**File:** `ChangeTracker.luau:45-46`
**Severity:** Low

The `instancePaths` table uses weak keys (good), but entries accumulate for destroyed instances until GC runs.

```lua
local instancePaths: {[Instance]: string} = {}
setmetatable(instancePaths, { __mode = "k" })
```

**Status:** Properly handled with weak keys.

---

### Grace period paths cleanup (MEM-003)
**File:** `ChangeTracker.luau:58-59, 105-112`
**Severity:** Low

The `recentFileWatcherPaths` and `recentlyApplied` tables are only cleaned when `shouldIgnoreChange()` is called, not on a regular interval.

```lua
local function cleanupExpiredPaths()
    -- Only called from shouldIgnoreChange
```

**Recommendation:** Add periodic cleanup or use a time-based eviction structure.

---

## 4. Race Conditions

### Extraction/sync race (RACE-001)
**File:** `init.server.luau:1633-1640`
**Severity:** Medium

The check for `isExtracting` and subsequent skip isn't atomic:

```lua
elseif isSyncCommand and isExtracting then
    -- Sync command could slip through between check and flag set
```

**Impact:** Sync operations could execute during extraction, causing data inconsistency.

**Recommendation:** Use a single operation lock or mutex pattern.

---

### Concurrent button clicks (RACE-002)
**File:** `init.server.luau:2148`
**Severity:** Low

The connect button has no debouncing - rapid clicks could trigger multiple connection attempts.

**Recommendation:** Add click debouncing or disable button during connection attempt.

---

### CSG clone cleanup (RACE-003)
**File:** `init.server.luau:766-831`
**Severity:** Low

If extraction fails after cloning a union but before cleanup, orphaned clones remain:

```lua
local unionClone = instance:Clone()
unionClone.Parent = instance.Parent
local separated = CSGHandler.separate(plugin, unionClone :: BasePart)
-- If failure occurs here, clone isn't cleaned up
```

**Recommendation:** Use try-finally pattern to ensure clone cleanup.

---

## 5. Error Handling Gaps

### Silent pcall failures (ERR-001)
**File:** `Serializer.luau:448-464`
**Severity:** Medium

Property read failures are silently swallowed after the first 3:

```lua
if not ok then
    errorCount += 1
    if errorCount <= 3 then
        -- Only warn for first few errors
    end
end
```

**Recommendation:** Log all failures in debug mode or to a summary.

---

### HTTP error messages (ERR-002)
**File:** `init.server.luau:471-512`
**Severity:** Low

HTTP failures return generic error messages without details:

```lua
return false, result  -- 'result' is often just an error string
```

**Recommendation:** Provide more context (status code, endpoint) in error messages.

---

### CSG reconstruction partial failure (ERR-003)
**File:** `CSGHandler.luau:152-197`
**Severity:** Low

If `SubtractAsync` fails after `UnionAsync` succeeds, the intermediate union is cleaned up but the original parts aren't restored.

**Recommendation:** Consider transaction-style rollback on failure.

---

## 6. Files Requiring Most Attention

| File | Priority | Issues | Reason |
|------|----------|--------|--------|
| `ChangeTracker.luau` | **High** | MEM, PERF, RACE | Event connection leak, unbounded growth |
| `Sync.luau` | **Medium** | MEM | Multiple unbounded caches |
| `init.server.luau` | **Medium** | RACE, ERR | Race conditions in command handling |
| `CSGHandler.luau` | **Low** | ERR | Partial failure cleanup |
| `Serializer.luau` | **Low** | PERF, ERR | Could benefit from caching |
| `Config.luau` | **Low** | SEC | URL validation needed |
| `Reflection.luau` | **Low** | - | Well-structured, no issues |
| `TerrainHandler.luau` | **Low** | - | Good error handling, proper yielding |

---

## 7. Code Quality Observations

### Positive Aspects
- Consistent use of `--!strict` type checking
- Good separation of concerns between modules
- Proper use of `task.wait()` for yielding in loops
- Waypoints for undo/redo support (`ChangeHistoryService`)
- Echo prevention pattern (`isSyncingFromServer` flag)
- Feature flagging for Creator Store vs GitHub versions

### Areas for Improvement
- Consider using a state machine for connection/sync states instead of multiple boolean flags
- Some functions exceed 100 lines and could be broken down
- Debug logging could use a configurable log level system
- Unit tests appear to exist (`TestRunner.luau`, `TestAssertions.luau`) but coverage is unclear

---

## 8. Recommendations Summary

### Immediate (Before v1.2)
1. Add pending changes limit in `ChangeTracker`
2. Implement periodic cache cleanup in `Sync`
3. Add extraction/sync mutual exclusion

### Short-term
1. Refactor `ChangeTracker` to use service-level events instead of per-instance
2. Add URL validation in `Config`
3. Improve error logging granularity

### Long-term
1. Implement configurable log levels
2. Consider WebSocket instead of polling
3. Add memory usage monitoring/limits

---

*Report generated by automated security review. Manual verification recommended for all findings.*
