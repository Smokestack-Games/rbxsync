-- sync-test.luau
-- Example: Testing that a synced script exists with correct content.
-- This runs during a playtest via `run_test` or `run_code`.
--
-- Usage: Sync this file into ServerScriptService, then start a playtest.

local ReplicatedStorage = game:GetService("ReplicatedStorage")
local ServerScriptService = game:GetService("ServerScriptService")

-- Inline TestLogger (for scripts that can't require the plugin module)
-- In practice, copy this block or require the plugin's TestLogger.
local TestLogger = require(game:GetService("CoreGui"):FindFirstChild("RbxSync").TestLogger)

TestLogger.begin("sync_verification")

-- Test 1: Verify a synced module exists
local utils = ReplicatedStorage:FindFirstChild("Utils")
TestLogger.assertNotNil("utils_module_exists", utils, "Utils module should exist in ReplicatedStorage")

-- Test 2: Verify script type
if utils then
	TestLogger.assert("utils_is_module", "ModuleScript", utils.ClassName)
end

-- Test 3: Verify a server script exists
local mainScript = ServerScriptService:FindFirstChild("Main")
TestLogger.assertNotNil("main_script_exists", mainScript, "Main script should exist in SSS")

-- Test 4: Check script class
if mainScript then
	TestLogger.assert("main_is_script", "Script", mainScript.ClassName)
end

-- Test 5: Snapshot the state of a key instance
if utils then
	TestLogger.snapshot("utils_module", utils)
end

TestLogger.finish()
