# RbxSync Test Coverage Analysis

**Date:** 2026-01-16
**Analyzed by:** Claude Agent
**Current Version:** v1.1.2

---

## Executive Summary

RbxSync has **minimal test coverage**. Only `rbxsync-core` has unit tests (~23 tests across 7 files). The critical sync engine (`rbxsync-server`), MCP server, CLI, VS Code extension, and Studio plugin have **zero automated tests**.

| Component | Test Files | Test Count | Coverage Status |
|-----------|-----------|------------|-----------------|
| rbxsync-core | 7 | ~23 | **Partial** |
| rbxsync-server | 0 | 0 | **None** |
| rbxsync-mcp | 0 | 0 | **None** |
| rbxsync-cli | 0 | 0 | **None** |
| plugin (Luau) | 0 | 0 | **None** (has infrastructure) |
| rbxsync-vscode | 0 | 0 | **None** |

---

## 1. Current Test Inventory

### rbxsync-core (23 tests across 7 files)

#### `path_utils.rs` - 5 tests
| Test | What It Covers |
|------|----------------|
| `test_normalize_path` | Windows backslash → forward slash conversion |
| `test_path_to_string` | PathBuf to string conversion |
| `test_path_with_suffix` | Appending suffix to path string |
| `test_pathbuf_with_suffix` | Appending suffix to PathBuf |
| `test_sanitize_filename` | Invalid filename character stripping |

#### `types/project.rs` - 2 tests
| Test | What It Covers |
|------|----------------|
| `test_default_config` | Default ProjectConfig values |
| `test_config_serialization` | JSON round-trip for config |

#### `types/wally.rs` - 3 tests
| Test | What It Covers |
|------|----------------|
| `test_is_package_path` | Wally package path detection |
| `test_parse_wally_manifest` | wally.toml parsing |
| `test_parse_wally_lock` | wally.lock parsing |

#### `rojo.rs` - 4 tests
| Test | What It Covers |
|------|----------------|
| `test_parse_simple_project` | Basic Rojo project.json parsing |
| `test_rojo_to_tree_mapping` | Tree → filesystem path mapping |
| `test_nested_paths` | Nested path resolution |
| `test_get_source_dir` | Source directory detection |

#### `plugin_builder.rs` - 4 tests
| Test | What It Covers |
|------|----------------|
| `test_parse_script_file_module` | ModuleScript detection from filename |
| `test_parse_script_file_server` | Script detection from filename |
| `test_parse_script_file_entry` | Entry script detection |
| `test_build_plugin` | Full plugin .rbxm generation |

#### `types/instance.rs` - 3 tests
| Test | What It Covers |
|------|----------------|
| `test_instance_creation` | Instance struct creation |
| `test_script_detection` | is_script() classification |
| `test_script_extensions` | File extension → script type |

#### `types/properties.rs` - 3 tests
| Test | What It Covers |
|------|----------------|
| `test_vector3_serialization` | Vector3 JSON round-trip |
| `test_cframe_serialization` | CFrame JSON round-trip |
| `test_enum_serialization` | EnumValue JSON round-trip |

### Plugin Testing Infrastructure (exists but unused)

The plugin has testing infrastructure files but **no actual tests**:
- `TestAssertions.luau` - Comprehensive assertion library
- `TestRunner.luau` - Test execution framework

---

## 2. Coverage Gaps by Component

### rbxsync-server (CRITICAL - 0 tests)

The server is the **core sync engine** with 40+ HTTP endpoints and complex state management.

| Area | Risk Level | What's Missing |
|------|------------|----------------|
| HTTP handlers | **P0** | No tests for any of 40+ endpoints |
| Extraction pipeline | **P0** | Chunked extraction, finalization |
| Sync logic | **P0** | read-tree, incremental sync, batch operations |
| File watcher | **P1** | Change detection, debouncing |
| Bot controller | **P1** | Playtest automation commands |
| Console streaming | **P2** | Broadcast, history, subscribe |
| Git operations | **P2** | Status, commit, init wrappers |

**Critical untested functions:**
- `apply_tree_mapping()` - Path resolution (bug history: RBXSYNC-17)
- `apply_reverse_tree_mapping()` - Reverse path lookup
- `handle_extract_*` endpoints - Extraction pipeline
- `handle_sync_*` endpoints - Sync operations
- `handle_bot_*` endpoints - Bot controller

### rbxsync-mcp (HIGH - 0 tests)

| Area | Risk Level | What's Missing |
|------|------------|----------------|
| RbxSyncClient | **P1** | HTTP client wrapper |
| Tool handlers | **P1** | MCP tool implementations |
| Parameter validation | **P1** | Schema validation |
| Error handling | **P2** | Error translation to MCP format |

### rbxsync-cli (MEDIUM - 0 tests)

| Area | Risk Level | What's Missing |
|------|------------|----------------|
| Argument parsing | **P2** | clap integration |
| Command execution | **P2** | serve, extract subcommands |
| iMessage integration | **P3** | Optional feature |

### plugin (Luau) (HIGH - 0 tests)

| Area | Risk Level | What's Missing |
|------|------------|----------------|
| Serializer | **P0** | Property encoding (all Roblox types) |
| Sync module | **P0** | Instance creation, property application |
| CSGHandler | **P1** | Union/Negate reconstruction |
| ChangeTracker | **P1** | Change detection |
| Reflection | **P2** | API dump parsing |

**Critical untested functions:**
- `Serializer.encodeValue()` - 30+ Roblox types
- `Sync.decodeValue()` - Inverse of encoding
- `Serializer.buildDisambiguatedPaths()` - Path disambiguation (RBXSYNC-25)
- `CSGHandler.reconstructUnion()` - CSG reconstruction (RBXSYNC-38)

### rbxsync-vscode (MEDIUM - 0 tests)

| Area | Risk Level | What's Missing |
|------|------------|----------------|
| RbxSyncClient | **P2** | HTTP client |
| Commands | **P2** | connect, extract, sync handlers |
| Webview | **P3** | React components |
| Status bar | **P3** | UI state management |

---

## 3. Recommended Tests to Add (Prioritized)

### P0 - Critical (Prevents Data Loss / Core Functionality)

#### 1. Server: Extraction Pipeline
```
rbxsync-server/tests/extraction_test.rs
- test_extract_start_creates_session
- test_extract_chunk_accumulates_data
- test_extract_finalize_writes_files
- test_extract_with_excluded_services (RBXSYNC-30)
- test_extract_clears_src_folder (RBXSYNC-27)
- test_extract_preserves_terrain
- test_extract_handles_large_games (RBXSYNC-26)
```

#### 2. Server: Sync Operations
```
rbxsync-server/tests/sync_test.rs
- test_sync_read_tree_returns_instances
- test_sync_incremental_detects_changes
- test_sync_batch_applies_operations
- test_sync_batch_deduplication (RBXSYNC-35)
- test_sync_respects_paused_flag
- test_sync_with_tree_mapping
```

#### 3. Core: Property Serialization (expand coverage)
```
rbxsync-core/tests/properties_test.rs
- test_all_vector_types (Vector2, Vector3, Vector2int16, Vector3int16)
- test_color_types (Color3, Color3uint8, BrickColor)
- test_udim_types (UDim, UDim2)
- test_sequence_types (NumberSequence, ColorSequence)
- test_instance_references
- test_font_serialization
- test_optional_cframe_serialization
```

### P1 - High Priority (Affects Reliability)

#### 4. Server: Path Handling
```
rbxsync-server/tests/path_test.rs
- test_normalize_path_windows (RBXSYNC-17)
- test_apply_tree_mapping_simple
- test_apply_tree_mapping_nested
- test_apply_reverse_tree_mapping
- test_tree_mapping_with_special_chars
```

#### 5. Server: File Watcher
```
rbxsync-server/tests/file_watcher_test.rs
- test_file_change_detection
- test_debounce_rapid_changes
- test_ignore_non_source_files
- test_echo_prevention (RBXSYNC-34)
```

#### 6. Plugin: Serializer (Luau tests)
```
plugin/tests/Serializer.spec.luau
- test_encode_primitives
- test_encode_vector_types
- test_encode_cframe
- test_encode_enums
- test_encode_instance_references
- test_buildDisambiguatedPaths_handles_duplicates
- test_getInstanceUUID_stable (RBXSYNC-36)
```

#### 7. Plugin: Sync Module (Luau tests)
```
plugin/tests/Sync.spec.luau
- test_decode_all_property_types
- test_apply_instance_properties
- test_create_instance_from_data
- test_resolve_pending_references
- test_handle_csg_reconstruction
```

### P2 - Medium Priority (Improves Confidence)

#### 8. MCP: Tool Handlers
```
rbxsync-mcp/tests/tools_test.rs
- test_extract_game_tool
- test_sync_to_studio_tool
- test_git_status_tool
- test_bot_observe_tool
- test_error_translation
```

#### 9. Server: Git Operations
```
rbxsync-server/tests/git_test.rs
- test_git_status_parses_output
- test_git_commit_creates_commit
- test_git_init_creates_repo
- test_git_log_parses_commits
```

#### 10. Server: Bot Controller
```
rbxsync-server/tests/bot_test.rs
- test_bot_command_queue
- test_bot_state_updates
- test_playtest_lifecycle
- test_bot_observe_types
```

### P3 - Lower Priority (Nice to Have)

#### 11. VS Code Extension
```
rbxsync-vscode/src/test/extension.test.ts
- test_activation
- test_client_connection
- test_extract_command
- test_sync_command
```

#### 12. CLI
```
rbxsync-cli/tests/cli_test.rs
- test_serve_command_starts_server
- test_extract_command_with_args
```

---

## 4. Testing Strategy for v1.3

### Phase 1: Unit Test Foundation (2-3 weeks)

1. **Set up test infrastructure**
   - Configure `cargo test` for all Rust crates
   - Set up mock HTTP server for server tests
   - Create test fixtures for common scenarios

2. **Add P0 tests** (critical paths)
   - Extraction pipeline tests
   - Sync operation tests
   - Property serialization tests

3. **Add P1 tests** (reliability)
   - Path handling tests
   - File watcher tests

### Phase 2: Integration Tests (1-2 weeks)

1. **Server integration tests**
   ```
   rbxsync-server/tests/integration/
   - full_extraction_flow.rs
   - full_sync_flow.rs
   - multi_workspace.rs
   ```

2. **MCP integration tests**
   ```
   rbxsync-mcp/tests/integration/
   - mcp_tool_flow.rs
   ```

### Phase 3: E2E Tests (2-3 weeks)

1. **Create test fixtures**
   - Sample Roblox places (.rbxl files)
   - Expected extraction outputs
   - Modification scenarios

2. **E2E test scenarios**
   ```
   tests/e2e/
   - extract_and_verify.rs
   - modify_and_sync.rs
   - concurrent_workspaces.rs
   - csg_roundtrip.rs
   ```

3. **Plugin E2E tests (Luau)**
   - Run in actual Studio environment
   - Use existing TestAssertions infrastructure
   - Automate via `run_test` MCP tool

### CI/CD Integration

1. **GitHub Actions workflow**
   ```yaml
   # .github/workflows/test.yml
   jobs:
     unit-tests:
       - cargo test --all
     integration-tests:
       - cargo test --test integration
     # E2E requires Studio - manual trigger
   ```

2. **Coverage reporting**
   - Use `cargo-tarpaulin` or `llvm-cov`
   - Set coverage thresholds (target: 60% for v1.3)

---

## 5. Test Debt Estimation

| Component | Estimated Tests Needed | Effort |
|-----------|----------------------|--------|
| rbxsync-server | 40-50 | 3-4 days |
| rbxsync-core (expand) | 15-20 | 1 day |
| rbxsync-mcp | 10-15 | 1 day |
| plugin (Luau) | 20-30 | 2 days |
| Integration tests | 10-15 | 2 days |
| E2E tests | 5-10 | 2-3 days |
| **Total** | **100-140 tests** | **11-15 days** |

---

## 6. Quick Wins

Tests that provide high value with minimal effort:

1. **Path normalization tests** (RBXSYNC-17 regression)
2. **Tree mapping tests** (critical for sync correctness)
3. **Property round-trip tests** (extend existing tests)
4. **Extraction session tests** (state machine)
5. **Plugin Serializer tests** (use existing TestAssertions)

---

## Appendix: Test File Locations

```
rbxsync/
├── rbxsync-core/
│   └── src/
│       ├── path_utils.rs       # Has #[cfg(test)] module
│       ├── types/
│       │   ├── instance.rs     # Has #[cfg(test)] module
│       │   ├── project.rs      # Has #[cfg(test)] module
│       │   ├── properties.rs   # Has #[cfg(test)] module
│       │   └── wally.rs        # Has #[cfg(test)] module
│       ├── rojo.rs             # Has #[cfg(test)] module
│       └── plugin_builder.rs   # Has #[cfg(test)] module
├── rbxsync-server/
│   └── tests/                  # MISSING - create this
├── rbxsync-mcp/
│   └── tests/                  # MISSING - create this
├── plugin/
│   └── tests/                  # MISSING - create this
│       └── src/
│           ├── TestAssertions.luau  # Infrastructure exists
│           └── TestRunner.luau      # Infrastructure exists
└── rbxsync-vscode/
    └── src/test/               # MISSING - create this
```

---

*Report generated by Claude Agent - 2026-01-16*
