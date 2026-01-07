--!strict
-- TestRunner.luau
-- Automated console capture for AI-powered development workflows
-- Enables AI agents to test their code changes and iterate based on console output

local TestRunner = {}

local LogService = game:GetService("LogService")
local RunService = game:GetService("RunService")
local StudioTestService = game:GetService("StudioTestService")

-- Types
export type ConsoleMessage = {
	message: string,
	type: string,
	timestamp: number,
}

export type CaptureResult = {
	success: boolean,
	output: { ConsoleMessage },
	totalMessages: number,
	duration: number?,
	error: string?,
}

-- Internal state
local isCapturing = false
local capturedOutput: { ConsoleMessage } = {}
local logConnection: RBXScriptConnection? = nil
local captureStartTime: number = 0

-- Start capturing console output
-- Call this before triggering a play session to capture all output
function TestRunner.startCapture(): { success: boolean, message: string }
	if isCapturing then
		return { success = false, message = "Capture already in progress" }
	end

	-- Reset state
	capturedOutput = {}
	captureStartTime = os.clock()
	isCapturing = true

	-- Connect to LogService to capture all output
	logConnection = LogService.MessageOut:Connect(function(message: string, messageType: Enum.MessageType)
		if isCapturing then
			table.insert(capturedOutput, {
				message = message,
				type = messageType.Name,
				timestamp = os.clock() - captureStartTime,
			})
		end
	end)

	return { success = true, message = "Capture started" }
end

-- Stop capturing and return all captured output
-- Call this after the play session ends to get results
function TestRunner.stopCapture(): CaptureResult
	if not isCapturing then
		return {
			success = false,
			output = {},
			totalMessages = 0,
			error = "No capture in progress",
		}
	end

	isCapturing = false
	local duration = os.clock() - captureStartTime

	if logConnection then
		logConnection:Disconnect()
		logConnection = nil
	end

	local output = capturedOutput
	capturedOutput = {}

	return {
		success = true,
		output = output,
		totalMessages = #output,
		duration = duration,
	}
end

-- Get current captured output without stopping
-- Useful for polling during long test sessions
function TestRunner.getOutput(): { capturing: boolean, output: { ConsoleMessage }, totalMessages: number }
	return {
		capturing = isCapturing,
		output = capturedOutput,
		totalMessages = #capturedOutput,
	}
end

-- Get capture status
function TestRunner.isCapturing(): boolean
	return isCapturing
end

-- Clear captured output without stopping capture
function TestRunner.clearOutput(): { success: boolean, clearedCount: number }
	local count = #capturedOutput
	capturedOutput = {}
	return { success = true, clearedCount = count }
end

-- Get historical log messages (what's already in the output window)
function TestRunner.getLogHistory(): { success: boolean, output: { ConsoleMessage } }
	local history = LogService:GetLogHistory()
	local output: { ConsoleMessage } = {}

	for _, entry in ipairs(history) do
		table.insert(output, {
			message = entry.message,
			type = entry.messageType.Name,
			timestamp = entry.timestamp,
		})
	end

	return {
		success = true,
		output = output,
	}
end

-- Filter captured output by message type
function TestRunner.filterOutput(messageType: string): { ConsoleMessage }
	local filtered: { ConsoleMessage } = {}
	for _, msg in ipairs(capturedOutput) do
		if msg.type == messageType then
			table.insert(filtered, msg)
		end
	end
	return filtered
end

-- Get error messages only (useful for quick failure detection)
function TestRunner.getErrors(): { ConsoleMessage }
	return TestRunner.filterOutput("MessageError")
end

-- Get warning messages only
function TestRunner.getWarnings(): { ConsoleMessage }
	return TestRunner.filterOutput("MessageWarning")
end

-- =============================================================================
-- PLAY MODE CONTROL (using StudioTestService for automated testing)
-- Uses ExecutePlayModeAsync / ExecuteRunModeAsync (December 2025 API)
-- =============================================================================

export type TestMode = "Play" | "Run"

export type TestOptions = {
	mode: TestMode?,      -- "Play" (solo) or "Run" (server simulation), default "Play"
	duration: number?,    -- How long to run (seconds), default 5
	testArg: any?,        -- Optional argument to pass to test session
}

-- Background test state
local testInProgress = false
local testComplete = false
local testError: string? = nil
local testResult: any = nil

-- Check if game is currently running
function TestRunner.isRunning(): boolean
	return RunService:IsRunning()
end

-- Start an automated test in the background (non-blocking)
-- Returns immediately, use getTestStatus() to poll for completion
function TestRunner.startBackgroundTest(options: TestOptions?): { success: boolean, message: string }
	local duration = (options and options.duration) or 5
	local mode = (options and options.mode) or "Play"
	local testArg = options and options.testArg

	if testInProgress then
		return { success = false, message = "Test already in progress" }
	end

	if RunService:IsRunning() then
		return { success = false, message = "Game already running" }
	end

	-- Reset state
	testInProgress = true
	testComplete = false
	testError = nil
	testResult = nil

	-- Start capture
	local captureResult = TestRunner.startCapture()
	if not captureResult.success then
		testInProgress = false
		return { success = false, message = captureResult.message }
	end

	-- Create auto-stop script that runs inside the game
	local autoStopScript = Instance.new("Script")
	autoStopScript.Name = "_RbxSyncAutoStop"
	autoStopScript.Source = string.format([[
local StudioTestService = game:GetService("StudioTestService")
task.delay(%d, function()
	StudioTestService:EndTest({ autoStopped = true })
end)
]], duration)
	autoStopScript.Parent = game:GetService("ServerScriptService")

	-- Spawn the test in a coroutine so it doesn't block
	task.spawn(function()
		-- Execute the test (this yields until EndTest is called from within the game)
		local ok, result = pcall(function()
			if mode == "Run" then
				return StudioTestService:ExecuteRunModeAsync(testArg or { duration = duration })
			else
				return StudioTestService:ExecutePlayModeAsync(testArg or { duration = duration })
			end
		end)

		-- Clean up the auto-stop script after test ends
		if autoStopScript and autoStopScript.Parent then
			autoStopScript:Destroy()
		end

		-- Test finished
		task.wait(0.5) -- Let final output come through

		if ok then
			testResult = result
		else
			testError = tostring(result)
		end

		testComplete = true
		testInProgress = false
	end)

	return { success = true, message = "Test started in background" }
end

-- Get status of background test
function TestRunner.getTestStatus(): {
	inProgress: boolean,
	complete: boolean,
	error: string?,
	result: any?,
	output: { ConsoleMessage },
	totalMessages: number
}
	return {
		inProgress = testInProgress,
		complete = testComplete,
		error = testError,
		result = testResult,
		output = capturedOutput,
		totalMessages = #capturedOutput,
	}
end

-- Stop background test and get results
function TestRunner.stopBackgroundTest(): CaptureResult
	-- Always try to end the test if one was in progress
	if testInProgress or not testComplete then
		pcall(function()
			StudioTestService:EndTest({ stopped = true })
		end)
		task.wait(1)
	end

	testInProgress = false

	local result = TestRunner.stopCapture()
	result.success = testError == nil
	result.error = testError
	result.testResult = testResult

	-- Reset state
	testComplete = false
	testError = nil
	testResult = nil

	return result
end

return TestRunner
