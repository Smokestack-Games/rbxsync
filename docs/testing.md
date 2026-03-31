# Debug Logging & Test Verification

When writing features or fixing bugs, **always include structured debug output** so that test results can be verified automatically. Use the `TestLogger` module (`plugin/src/TestLogger.luau`) or follow the format manually.

## Log Format

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

## Using TestLogger

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

## Example Output

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

## When to Write Tests

- **Every feature**: Write at least one test that verifies the core behavior
- **Every bug fix**: Write a test that would have caught the bug
- **Before PR**: Run your tests via `run_test` and confirm `[TEST SUMMARY]` shows PASS
- **Property changes**: Use `snapshot()` before and after to show the diff

## Example Test Scripts

See `testing/examples/` for complete working examples:
- `sync-test.luau` — verifying synced instances exist with correct types
- `instance-properties-test.luau` — property assertions with before/after snapshots
- `event-test.luau` — event verification with argument checking
