--!strict
-- TestAssertions.luau
-- Helper functions for writing clear, expressive tests

local TestAssertions = {}

export type AssertionResult = {
	passed: boolean,
	message: string?,
	expected: any?,
	actual: any?,
}

-- Basic assertions

function TestAssertions.assertEqual(actual: any, expected: any, message: string?): AssertionResult
	local passed = actual == expected
	return {
		passed = passed,
		message = message or (passed and "Values are equal" or string.format("Expected %s but got %s", tostring(expected), tostring(actual))),
		expected = expected,
		actual = actual,
	}
end

function TestAssertions.assertNotEqual(actual: any, notExpected: any, message: string?): AssertionResult
	local passed = actual ~= notExpected
	return {
		passed = passed,
		message = message or (passed and "Values are not equal" or string.format("Expected value to differ from %s", tostring(notExpected))),
		expected = "not " .. tostring(notExpected),
		actual = actual,
	}
end

function TestAssertions.assertTrue(condition: boolean, message: string?): AssertionResult
	return {
		passed = condition,
		message = message or (condition and "Condition is true" or "Expected true but got false"),
		expected = true,
		actual = condition,
	}
end

function TestAssertions.assertFalse(condition: boolean, message: string?): AssertionResult
	return {
		passed = not condition,
		message = message or (not condition and "Condition is false" or "Expected false but got true"),
		expected = false,
		actual = condition,
	}
end

function TestAssertions.assertNil(value: any, message: string?): AssertionResult
	local passed = value == nil
	return {
		passed = passed,
		message = message or (passed and "Value is nil" or string.format("Expected nil but got %s", tostring(value))),
		expected = nil,
		actual = value,
	}
end

function TestAssertions.assertNotNil(value: any, message: string?): AssertionResult
	local passed = value ~= nil
	return {
		passed = passed,
		message = message or (passed and "Value is not nil" or "Expected non-nil value"),
		expected = "not nil",
		actual = value,
	}
end

-- Numeric assertions

function TestAssertions.assertGreater(actual: number, threshold: number, message: string?): AssertionResult
	local passed = actual > threshold
	return {
		passed = passed,
		message = message or (passed and "Value is greater" or string.format("Expected %s > %s", tostring(actual), tostring(threshold))),
		expected = "> " .. tostring(threshold),
		actual = actual,
	}
end

function TestAssertions.assertGreaterOrEqual(actual: number, threshold: number, message: string?): AssertionResult
	local passed = actual >= threshold
	return {
		passed = passed,
		message = message or (passed and "Value is greater or equal" or string.format("Expected %s >= %s", tostring(actual), tostring(threshold))),
		expected = ">= " .. tostring(threshold),
		actual = actual,
	}
end

function TestAssertions.assertLess(actual: number, threshold: number, message: string?): AssertionResult
	local passed = actual < threshold
	return {
		passed = passed,
		message = message or (passed and "Value is less" or string.format("Expected %s < %s", tostring(actual), tostring(threshold))),
		expected = "< " .. tostring(threshold),
		actual = actual,
	}
end

function TestAssertions.assertLessOrEqual(actual: number, threshold: number, message: string?): AssertionResult
	local passed = actual <= threshold
	return {
		passed = passed,
		message = message or (passed and "Value is less or equal" or string.format("Expected %s <= %s", tostring(actual), tostring(threshold))),
		expected = "<= " .. tostring(threshold),
		actual = actual,
	}
end

function TestAssertions.assertInRange(actual: number, min: number, max: number, message: string?): AssertionResult
	local passed = actual >= min and actual <= max
	return {
		passed = passed,
		message = message or (passed and "Value is in range" or string.format("Expected %s to be in range [%s, %s]", tostring(actual), tostring(min), tostring(max))),
		expected = string.format("[%s, %s]", tostring(min), tostring(max)),
		actual = actual,
	}
end

function TestAssertions.assertApproximately(actual: number, expected: number, epsilon: number?, message: string?): AssertionResult
	epsilon = epsilon or 0.001
	local passed = math.abs(actual - expected) <= epsilon
	return {
		passed = passed,
		message = message or (passed and "Values are approximately equal" or string.format("Expected %s to be approximately %s (within %s)", tostring(actual), tostring(expected), tostring(epsilon))),
		expected = expected,
		actual = actual,
	}
end

-- String assertions

function TestAssertions.assertContains(haystack: string, needle: string, message: string?): AssertionResult
	local passed = string.find(haystack, needle, 1, true) ~= nil
	return {
		passed = passed,
		message = message or (passed and "String contains substring" or string.format("Expected '%s' to contain '%s'", haystack, needle)),
		expected = "contains '" .. needle .. "'",
		actual = haystack,
	}
end

function TestAssertions.assertMatches(str: string, pattern: string, message: string?): AssertionResult
	local passed = string.match(str, pattern) ~= nil
	return {
		passed = passed,
		message = message or (passed and "String matches pattern" or string.format("Expected '%s' to match pattern '%s'", str, pattern)),
		expected = "matches '" .. pattern .. "'",
		actual = str,
	}
end

-- Table assertions

function TestAssertions.assertTableContains(tbl: {any}, value: any, message: string?): AssertionResult
	local found = false
	for _, v in ipairs(tbl) do
		if v == value then
			found = true
			break
		end
	end
	return {
		passed = found,
		message = message or (found and "Table contains value" or string.format("Expected table to contain %s", tostring(value))),
		expected = "contains " .. tostring(value),
		actual = tbl,
	}
end

function TestAssertions.assertTableHasKey(tbl: {[any]: any}, key: any, message: string?): AssertionResult
	local passed = tbl[key] ~= nil
	return {
		passed = passed,
		message = message or (passed and "Table has key" or string.format("Expected table to have key %s", tostring(key))),
		expected = "has key " .. tostring(key),
		actual = tbl,
	}
end

function TestAssertions.assertTableLength(tbl: {any}, expectedLength: number, message: string?): AssertionResult
	local actualLength = #tbl
	local passed = actualLength == expectedLength
	return {
		passed = passed,
		message = message or (passed and "Table has expected length" or string.format("Expected table length %d but got %d", expectedLength, actualLength)),
		expected = expectedLength,
		actual = actualLength,
	}
end

-- Async helpers

-- Wait for a condition to become true, with timeout
function TestAssertions.waitFor(
	condition: () -> boolean,
	timeout: number?,
	pollInterval: number?,
	message: string?
): AssertionResult
	timeout = timeout or 5
	pollInterval = pollInterval or 0.1

	local startTime = os.clock()

	while os.clock() - startTime < timeout do
		if condition() then
			return {
				passed = true,
				message = message or "Condition met",
				expected = true,
				actual = true,
			}
		end
		task.wait(pollInterval)
	end

	return {
		passed = false,
		message = message or string.format("Timeout after %ds waiting for condition", timeout),
		expected = true,
		actual = false,
	}
end

-- Wait for a value to change
function TestAssertions.waitForChange(
	getValue: () -> any,
	timeout: number?,
	pollInterval: number?,
	message: string?
): AssertionResult
	timeout = timeout or 5
	pollInterval = pollInterval or 0.1

	local initialValue = getValue()
	local startTime = os.clock()

	while os.clock() - startTime < timeout do
		local currentValue = getValue()
		if currentValue ~= initialValue then
			return {
				passed = true,
				message = message or "Value changed",
				expected = "changed from " .. tostring(initialValue),
				actual = currentValue,
			}
		end
		task.wait(pollInterval)
	end

	return {
		passed = false,
		message = message or string.format("Timeout after %ds waiting for value to change from %s", timeout, tostring(initialValue)),
		expected = "changed from " .. tostring(initialValue),
		actual = initialValue,
	}
end

-- Expect a value to change after performing an action
function TestAssertions.expectChange(
	getValue: () -> any,
	action: () -> (),
	predicate: ((old: any, new: any) -> boolean)?,
	message: string?
): AssertionResult
	local oldValue = getValue()

	action()

	-- Small delay for async operations
	task.wait(0.1)

	local newValue = getValue()

	local passed
	if predicate then
		passed = predicate(oldValue, newValue)
	else
		passed = oldValue ~= newValue
	end

	return {
		passed = passed,
		message = message or (passed and "Value changed as expected" or string.format("Expected value to change from %s, got %s", tostring(oldValue), tostring(newValue))),
		expected = "changed from " .. tostring(oldValue),
		actual = newValue,
	}
end

-- Test runner helper
function TestAssertions.runTest(name: string, testFn: () -> AssertionResult): {name: string, result: AssertionResult}
	local ok, result = pcall(testFn)

	if not ok then
		return {
			name = name,
			result = {
				passed = false,
				message = "Test threw an error: " .. tostring(result),
				expected = "no error",
				actual = tostring(result),
			}
		}
	end

	return {
		name = name,
		result = result,
	}
end

-- Run multiple tests and return summary
function TestAssertions.runTests(tests: {{name: string, fn: () -> AssertionResult}}): {
	passed: number,
	failed: number,
	results: {{name: string, result: AssertionResult}},
}
	local results = {}
	local passed = 0
	local failed = 0

	for _, test in ipairs(tests) do
		local testResult = TestAssertions.runTest(test.name, test.fn)
		table.insert(results, testResult)

		if testResult.result.passed then
			passed = passed + 1
		else
			failed = failed + 1
		end
	end

	return {
		passed = passed,
		failed = failed,
		results = results,
	}
end

-- Print test results (for console output)
function TestAssertions.printResults(summary: {passed: number, failed: number, results: {{name: string, result: AssertionResult}}})
	print("\n" .. string.rep("=", 50))
	print("     TEST RESULTS")
	print(string.rep("=", 50))

	for _, testResult in ipairs(summary.results) do
		local status = testResult.result.passed and "[PASS]" or "[FAIL]"
		print(status .. " " .. testResult.name)
		if not testResult.result.passed and testResult.result.message then
			print("       " .. testResult.result.message)
		end
	end

	print(string.rep("-", 50))
	print(string.format("Passed: %d | Failed: %d | Total: %d", summary.passed, summary.failed, summary.passed + summary.failed))
	print(string.rep("=", 50) .. "\n")
end

return TestAssertions
