--!strict
-- TestLogger.luau
-- Structured debug logging for AI-agent test verification.
-- Produces output an LLM can parse to determine PASS/FAIL unambiguously.
--
-- Format:
--   [TEST BEGIN] suite_name
--   [TEST:name] PASS
--   [TEST:name] PASS — description
--   [TEST:name] FAIL — expected: X, actual: Y
--   [TEST:name] FAIL — reason
--   [SNAPSHOT:label] ClassName=Part, Position=0,0,0, ...
--   [EVENT:name] fired — args: arg1, arg2
--   [TEST SUMMARY] 3 passed, 1 failed, 4 total — FAIL

local TestLogger = {}

-- Internal state
local passed = 0
local failed = 0
local suiteName: string? = nil
local results: { string } = {}

-- Helpers ----------------------------------------------------------------

local function formatValue(v: any): string
	if typeof(v) == "Instance" then
		return v:GetFullName()
	elseif typeof(v) == "Vector3" then
		return string.format("%.2f, %.2f, %.2f", v.X, v.Y, v.Z)
	elseif typeof(v) == "CFrame" then
		local p = v.Position
		return string.format("CFrame(%.2f, %.2f, %.2f)", p.X, p.Y, p.Z)
	elseif typeof(v) == "Color3" then
		return string.format("Color3(%.2f, %.2f, %.2f)", v.R, v.G, v.B)
	elseif typeof(v) == "UDim2" then
		return string.format("UDim2(%s, %d, %s, %d)", tostring(v.X.Scale), v.X.Offset, tostring(v.Y.Scale), v.Y.Offset)
	elseif typeof(v) == "EnumItem" then
		return tostring(v)
	elseif type(v) == "table" then
		local parts: { string } = {}
		for k, val in v do
			table.insert(parts, tostring(k) .. "=" .. formatValue(val))
		end
		return "{" .. table.concat(parts, ", ") .. "}"
	else
		return tostring(v)
	end
end

local function record(line: string)
	table.insert(results, line)
	print(line)
end

-- Public API --------------------------------------------------------------

--- Start a test suite. Resets counters.
function TestLogger.begin(name: string)
	suiteName = name
	passed = 0
	failed = 0
	results = {}
	record("[TEST BEGIN] " .. name)
end

--- Assert that actual == expected. Prints PASS or FAIL with values.
function TestLogger.assert(name: string, expected: any, actual: any, message: string?): boolean
	if actual == expected then
		local line = "[TEST:" .. name .. "] PASS"
		if message then
			line = line .. " \u{2014} " .. message
		end
		record(line)
		passed += 1
		return true
	else
		local line = "[TEST:" .. name .. "] FAIL \u{2014} expected: "
			.. formatValue(expected) .. ", actual: " .. formatValue(actual)
		if message then
			line = line .. " (" .. message .. ")"
		end
		record(line)
		failed += 1
		return false
	end
end

--- Assert a boolean condition is true.
function TestLogger.assertTrue(name: string, condition: boolean, message: string?): boolean
	if condition then
		local line = "[TEST:" .. name .. "] PASS"
		if message then
			line = line .. " \u{2014} " .. message
		end
		record(line)
		passed += 1
		return true
	else
		local line = "[TEST:" .. name .. "] FAIL \u{2014} expected: true, actual: false"
		if message then
			line = line .. " (" .. message .. ")"
		end
		record(line)
		failed += 1
		return false
	end
end

--- Assert a boolean condition is false.
function TestLogger.assertFalse(name: string, condition: boolean, message: string?): boolean
	return TestLogger.assertTrue(name, not condition, message)
end

--- Assert a value is not nil.
function TestLogger.assertNotNil(name: string, value: any, message: string?): boolean
	if value ~= nil then
		local line = "[TEST:" .. name .. "] PASS"
		if message then
			line = line .. " \u{2014} " .. message
		end
		record(line)
		passed += 1
		return true
	else
		local line = "[TEST:" .. name .. "] FAIL \u{2014} expected: non-nil, actual: nil"
		if message then
			line = line .. " (" .. message .. ")"
		end
		record(line)
		failed += 1
		return false
	end
end

--- Assert a numeric value is within a tolerance of expected.
function TestLogger.assertApprox(name: string, expected: number, actual: number, epsilon: number?, message: string?): boolean
	local eps = epsilon or 0.01
	if math.abs(actual - expected) <= eps then
		local line = "[TEST:" .. name .. "] PASS"
		if message then
			line = line .. " \u{2014} " .. message
		end
		record(line)
		passed += 1
		return true
	else
		local line = "[TEST:" .. name .. "] FAIL \u{2014} expected: ~"
			.. formatValue(expected) .. " (±" .. formatValue(eps) .. "), actual: " .. formatValue(actual)
		if message then
			line = line .. " (" .. message .. ")"
		end
		record(line)
		failed += 1
		return false
	end
end

--- Record an explicit pass.
function TestLogger.pass(name: string, description: string?)
	local line = "[TEST:" .. name .. "] PASS"
	if description then
		line = line .. " \u{2014} " .. description
	end
	record(line)
	passed += 1
end

--- Record an explicit fail.
function TestLogger.fail(name: string, reason: string)
	record("[TEST:" .. name .. "] FAIL \u{2014} " .. reason)
	failed += 1
end

--- Snapshot an Instance's key properties for before/after comparison.
function TestLogger.snapshot(label: string, instance: Instance)
	local props: { string } = {}
	table.insert(props, "ClassName=" .. instance.ClassName)
	table.insert(props, "Name=" .. instance.Name)

	-- Common properties (pcall each since not all classes have them)
	local tryProp = function(propName: string)
		local ok, val = pcall(function()
			return (instance :: any)[propName]
		end)
		if ok and val ~= nil then
			table.insert(props, propName .. "=" .. formatValue(val))
		end
	end

	tryProp("Position")
	tryProp("Size")
	tryProp("CFrame")
	tryProp("Anchored")
	tryProp("CanCollide")
	tryProp("Transparency")
	tryProp("Color")
	tryProp("Material")
	tryProp("Health")
	tryProp("MaxHealth")
	tryProp("WalkSpeed")
	tryProp("Value")
	tryProp("Enabled")
	tryProp("Visible")
	tryProp("Text")

	record("[SNAPSHOT:" .. label .. "] " .. table.concat(props, ", "))
end

--- Log a verified event firing.
function TestLogger.event(name: string, args: { any }?)
	local line = "[EVENT:" .. name .. "] fired"
	if args and #args > 0 then
		local formatted: { string } = {}
		for _, a in args do
			table.insert(formatted, formatValue(a))
		end
		line = line .. " \u{2014} args: " .. table.concat(formatted, ", ")
	end
	record(line)
end

--- Print the final summary. Returns true if all tests passed.
function TestLogger.finish(): boolean
	local total = passed + failed
	local status = if failed == 0 then "PASS" else "FAIL"
	local line = "[TEST SUMMARY] "
		.. tostring(passed) .. " passed, "
		.. tostring(failed) .. " failed, "
		.. tostring(total) .. " total \u{2014} " .. status
	record(line)
	return failed == 0
end

--- Get current results (for programmatic access).
function TestLogger.getResults(): { passed: number, failed: number, total: number, lines: { string } }
	return {
		passed = passed,
		failed = failed,
		total = passed + failed,
		lines = results,
	}
end

--- Reset all state without printing.
function TestLogger.reset()
	passed = 0
	failed = 0
	suiteName = nil
	results = {}
end

return TestLogger
