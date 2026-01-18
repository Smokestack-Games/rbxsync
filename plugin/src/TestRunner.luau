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

-- Bot injection state
local injectedBotController: ModuleScript? = nil
local injectedBotRunnerServer: Script? = nil
local injectedBotRunnerClient: LocalScript? = nil
local injectedRemoteEvent: RemoteEvent? = nil
local injectedRemoteFunction: RemoteFunction? = nil

-- Check if game is currently running
function TestRunner.isRunning(): boolean
	return RunService:IsRunning()
end

-- Inject bot controller and runner for AI-powered testing
local function injectBotScripts()
	local ReplicatedStorage = game:GetService("ReplicatedStorage")
	local ServerScriptService = game:GetService("ServerScriptService")
	local StarterPlayer = game:GetService("StarterPlayer")
	local StarterPlayerScripts = StarterPlayer:FindFirstChild("StarterPlayerScripts")

	-- Try to enable LoadStringEnabled for server-side code execution (queryServer)
	-- This property may not be accessible from plugin edit-mode context
	-- If it fails here, the BotRunnerServer will try to enable it at runtime
	pcall(function()
		if not ServerScriptService.LoadStringEnabled then
			ServerScriptService.LoadStringEnabled = true
			print("[TestRunner] Enabled LoadStringEnabled for bot testing")
		end
	end)

	-- Clean up any existing injected scripts
	if injectedBotController and injectedBotController.Parent then
		injectedBotController:Destroy()
	end
	if injectedBotRunnerServer and injectedBotRunnerServer.Parent then
		injectedBotRunnerServer:Destroy()
	end
	if injectedBotRunnerClient and injectedBotRunnerClient.Parent then
		injectedBotRunnerClient:Destroy()
	end
	if injectedRemoteEvent and injectedRemoteEvent.Parent then
		injectedRemoteEvent:Destroy()
	end
	if injectedRemoteFunction and injectedRemoteFunction.Parent then
		injectedRemoteFunction:Destroy()
	end

	-- Also clean up any orphaned scripts from previous runs
	local existingController = ReplicatedStorage:FindFirstChild("BotController")
	if existingController then
		existingController:Destroy()
	end
	local existingServerRunner = ServerScriptService:FindFirstChild("BotRunnerServer")
	if existingServerRunner then
		existingServerRunner:Destroy()
	end
	local existingClientRunner = StarterPlayerScripts and StarterPlayerScripts:FindFirstChild("BotRunnerClient")
	if existingClientRunner then
		existingClientRunner:Destroy()
	end
	local existingRemote = ReplicatedStorage:FindFirstChild("BotCommandRemote")
	if existingRemote then
		existingRemote:Destroy()
	end
	local existingStateRemote = ReplicatedStorage:FindFirstChild("BotStateRemote")
	if existingStateRemote then
		existingStateRemote:Destroy()
	end

	-- Get the source modules from the plugin
	local pluginScript = script.Parent
	local botControllerSource = pluginScript:FindFirstChild("BotController")
	local botRunnerServerSourceModule = pluginScript:FindFirstChild("BotRunnerServerSource")
	local botRunnerClientSourceModule = pluginScript:FindFirstChild("BotRunnerClientSource")

	-- Create RemoteEvent for server-client communication
	injectedRemoteEvent = Instance.new("RemoteEvent")
	injectedRemoteEvent.Name = "BotCommandRemote"
	injectedRemoteEvent.Parent = ReplicatedStorage
	print("[TestRunner] Created BotCommandRemote in ReplicatedStorage")

	-- Create RemoteFunction for state queries
	injectedRemoteFunction = Instance.new("RemoteFunction")
	injectedRemoteFunction.Name = "BotStateRemote"
	injectedRemoteFunction.Parent = ReplicatedStorage
	print("[TestRunner] Created BotStateRemote in ReplicatedStorage")

	if botControllerSource and botControllerSource:IsA("ModuleScript") then
		-- Clone BotController to ReplicatedStorage
		injectedBotController = botControllerSource:Clone()
		injectedBotController.Parent = ReplicatedStorage
		print("[TestRunner] Injected BotController into ReplicatedStorage")
	else
		warn("[TestRunner] BotController module not found in plugin")
	end

	if botRunnerServerSourceModule and botRunnerServerSourceModule:IsA("ModuleScript") then
		-- Get source code from module and create Script
		local success, sourceCode = pcall(function()
			return require(botRunnerServerSourceModule)
		end)
		if success and sourceCode then
			injectedBotRunnerServer = Instance.new("Script")
			injectedBotRunnerServer.Name = "BotRunnerServer"
			injectedBotRunnerServer.Source = sourceCode
			injectedBotRunnerServer.Parent = ServerScriptService
			print("[TestRunner] Injected BotRunnerServer into ServerScriptService")
		else
			warn("[TestRunner] Failed to load BotRunnerServerSource:", sourceCode)
		end
	else
		warn("[TestRunner] BotRunnerServerSource module not found in plugin")
	end

	if botRunnerClientSourceModule and botRunnerClientSourceModule:IsA("ModuleScript") and StarterPlayerScripts then
		-- Get source code from module and create LocalScript
		local success, sourceCode = pcall(function()
			return require(botRunnerClientSourceModule)
		end)
		if success and sourceCode then
			injectedBotRunnerClient = Instance.new("LocalScript")
			injectedBotRunnerClient.Name = "BotRunnerClient"
			injectedBotRunnerClient.Source = sourceCode
			injectedBotRunnerClient.Parent = StarterPlayerScripts
			print("[TestRunner] Injected BotRunnerClient into StarterPlayerScripts")
		else
			warn("[TestRunner] Failed to load BotRunnerClientSource:", sourceCode)
		end
	else
		warn("[TestRunner] BotRunnerClientSource module not found in plugin or StarterPlayerScripts missing")
	end
end

-- Clean up injected bot scripts
local function cleanupBotScripts()
	if injectedBotController and injectedBotController.Parent then
		injectedBotController:Destroy()
		injectedBotController = nil
	end
	if injectedBotRunnerServer and injectedBotRunnerServer.Parent then
		injectedBotRunnerServer:Destroy()
		injectedBotRunnerServer = nil
	end
	if injectedBotRunnerClient and injectedBotRunnerClient.Parent then
		injectedBotRunnerClient:Destroy()
		injectedBotRunnerClient = nil
	end
	if injectedRemoteEvent and injectedRemoteEvent.Parent then
		injectedRemoteEvent:Destroy()
		injectedRemoteEvent = nil
	end
	if injectedRemoteFunction and injectedRemoteFunction.Parent then
		injectedRemoteFunction:Destroy()
		injectedRemoteFunction = nil
	end
end

-- Start an automated test in the background (non-blocking)
-- Returns immediately, use getTestStatus() to poll for completion
function TestRunner.startBackgroundTest(options: TestOptions?): { success: boolean, message: string }
	local duration = (options and options.duration) or 5
	local mode = (options and options.mode) or "Play"
	local testArg = options and options.testArg

	-- Auto-recover from stale state: if testInProgress but game isn't running, reset
	if testInProgress and not RunService:IsRunning() then
		warn("[TestRunner] Recovering from stale test state")
		testInProgress = false
		testComplete = false
		testError = nil
		testResult = nil
		if isCapturing then
			TestRunner.stopCapture()
		end
		cleanupBotScripts()
	end

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

	-- Inject bot scripts for AI-powered testing
	injectBotScripts()

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

		-- Clean up injected bot scripts
		cleanupBotScripts()

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

	-- Clean up injected bot scripts
	cleanupBotScripts()

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

-- Force-stop any running playtest immediately
-- This can be called from the plugin to terminate a test without waiting
function TestRunner.forceStopTest(): { success: boolean, message: string }
	local success, err = pcall(function()
		StudioTestService:EndTest({ forceStopped = true })
	end)

	if not success then
		return { success = false, message = "Failed to stop test: " .. tostring(err) }
	end

	-- Clean up capture state
	testInProgress = false
	testComplete = true

	-- Clean up injected bot scripts
	cleanupBotScripts()

	-- Stop capture if active
	if isCapturing then
		TestRunner.stopCapture()
	end

	return { success = true, message = "Test force stopped" }
end

return TestRunner
