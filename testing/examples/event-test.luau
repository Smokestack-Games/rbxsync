-- event-test.luau
-- Example: Testing that events fire correctly with expected arguments.
-- Demonstrates event verification and async waiting patterns.
--
-- Scenario: Agent implemented a door that opens when touched.

local Workspace = game:GetService("Workspace")

local TestLogger = require(game:GetService("CoreGui"):FindFirstChild("RbxSync").TestLogger)

TestLogger.begin("door_events")

local door = Workspace:FindFirstChild("Door")

if not TestLogger.assertNotNil("door_exists", door, "Door model should exist") then
	TestLogger.finish()
	return
end

-- Test 1: Door starts closed
local doorPart = door:FindFirstChild("DoorPart")
TestLogger.assertNotNil("door_part_exists", doorPart)

if doorPart then
	TestLogger.snapshot("door_initial", doorPart)
	TestLogger.assert("door_starts_closed", 0, doorPart.Transparency, "Door should be opaque when closed")
end

-- Test 2: Verify touch event wiring
local touchInterest = doorPart and doorPart:FindFirstChildWhichIsA("TouchTransmitter")
-- Note: TouchTransmitter is created automatically when .Touched is connected
TestLogger.assertTrue("touch_connected", touchInterest ~= nil, "Door should have Touched event connected")

-- Test 3: Simulate interaction and verify event
local eventFired = false
local eventArgs = {}

if doorPart then
	-- Listen for a custom event (e.g., BindableEvent the door fires)
	local doorOpenEvent = door:FindFirstChild("DoorOpened")
	if doorOpenEvent and doorOpenEvent:IsA("BindableEvent") then
		local conn = doorOpenEvent.Event:Connect(function(playerName: string)
			eventFired = true
			table.insert(eventArgs, playerName)
		end)

		-- Simulate the door opening
		doorOpenEvent:Fire("TestPlayer")
		task.wait(0.1)
		conn:Disconnect()

		TestLogger.assertTrue("door_event_fired", eventFired)
		TestLogger.event("DoorOpened", eventArgs)
	else
		TestLogger.fail("door_event_setup", "DoorOpened BindableEvent not found")
	end
end

-- Test 4: Verify door state after opening
if doorPart and eventFired then
	TestLogger.snapshot("door_after_open", doorPart)
end

TestLogger.finish()
