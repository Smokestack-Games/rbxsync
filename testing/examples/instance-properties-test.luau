-- instance-properties-test.luau
-- Example: Testing that instance properties are set correctly after sync.
-- Demonstrates before/after snapshots and property assertions.
--
-- Scenario: Agent created a Part with specific properties, wants to verify.

local Workspace = game:GetService("Workspace")

local TestLogger = require(game:GetService("CoreGui"):FindFirstChild("RbxSync").TestLogger)

TestLogger.begin("instance_properties")

-- Find the part we're testing
local testPart = Workspace:FindFirstChild("TestPart")

-- Test 1: Part exists
if not TestLogger.assertNotNil("part_exists", testPart, "TestPart should be in Workspace") then
	-- Early exit — remaining tests depend on the part
	TestLogger.fail("part_properties", "Skipped — TestPart not found")
	TestLogger.finish()
	return
end

-- Snapshot before any modifications
TestLogger.snapshot("before", testPart)

-- Test 2: Check basic properties
TestLogger.assert("part_classname", "Part", testPart.ClassName)
TestLogger.assert("part_anchored", true, testPart.Anchored)
TestLogger.assertTrue("part_can_collide", testPart.CanCollide, "Part should be collidable")

-- Test 3: Check position is approximately correct
TestLogger.assertApprox("part_pos_x", 10, testPart.Position.X)
TestLogger.assertApprox("part_pos_y", 5, testPart.Position.Y)
TestLogger.assertApprox("part_pos_z", 0, testPart.Position.Z)

-- Test 4: Check size
TestLogger.assert("part_size", Vector3.new(4, 1, 2), testPart.Size)

-- Test 5: Modify and verify change
testPart.Transparency = 0.5
TestLogger.assert("transparency_changed", 0.5, testPart.Transparency)

-- Snapshot after modifications
TestLogger.snapshot("after", testPart)

TestLogger.finish()
