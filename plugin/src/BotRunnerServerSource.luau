-- ModuleScript that returns the BotRunner server source code
-- This is injected as a Script into ServerScriptService during playtest

return [[
--!strict
-- BotRunnerServer
-- Server-side: Polls HTTP for commands and relays to client via RemoteEvent

local HttpService = game:GetService("HttpService")
local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")

local SERVER_URL = "http://127.0.0.1:44755"
local POLL_INTERVAL = 0.05 -- 50ms between polls (fast)
local STATE_REPORT_INTERVAL = 0.5 -- Report state every 500ms

print("[BotRunner:Server] Starting...")

-- Check HTTP access
local httpEnabled = false
local ok = pcall(function()
	local response = HttpService:RequestAsync({
		Url = SERVER_URL .. "/health",
		Method = "GET",
	})
	httpEnabled = response.Success
end)

if not httpEnabled then
	warn("=============================================")
	warn("[BotRunner:Server] HTTP REQUESTS ARE DISABLED!")
	warn("Enable: Game Settings > Security > Allow HTTP Requests")
	warn("=============================================")
	return
end

print("[BotRunner:Server] HTTP access confirmed!")

-- Enable LoadStringEnabled for server-side code execution (queryServer)
-- This is required for loadstring() to work in the queryServer command
local ServerScriptService = game:GetService("ServerScriptService")

local loadStringOk, loadStringErr = pcall(function()
	if not ServerScriptService.LoadStringEnabled then
		ServerScriptService.LoadStringEnabled = true
		print("[BotRunner:Server] Enabled LoadStringEnabled for queryServer")
	end
end)
if not loadStringOk then
	warn("[BotRunner:Server] Could not enable LoadStringEnabled:", loadStringErr)
	warn("[BotRunner:Server] queryServer commands may fail - enable manually in Game Settings > Security")
end

-- Send hello notification to server
pcall(function()
	HttpService:RequestAsync({
		Url = SERVER_URL .. "/bot/lifecycle",
		Method = "POST",
		Headers = { ["Content-Type"] = "application/json" },
		Body = HttpService:JSONEncode({
			event = "hello",
			timestamp = os.clock(),
		}),
	})
end)
print("[BotRunner:Server] Sent hello to server")

-- Get RemoteEvent for client communication (created by TestRunner)
local botRemote = ReplicatedStorage:WaitForChild("BotCommandRemote", 5)
if not botRemote then
	warn("[BotRunner:Server] BotCommandRemote not found!")
	return
end

-- Track connected players and their state
local playerStates = {}
local commandResults = {}

-- Handle command results from client
botRemote.OnServerEvent:Connect(function(player, data)
	if data.type == "result" and data.id then
		commandResults[data.id] = {
			result = data.result,
			state = data.state,
			player = player.Name,
		}
	elseif data.type == "state" then
		playerStates[player] = data.state
	end
end)

-- Get combined state from all players (or first player for single-player)
local function getState()
	for player, state in pairs(playerStates) do
		if state then
			return state
		end
	end
	return {}
end

-- Poll for pending commands
local function pollForCommands()
	local ok, response = pcall(function()
		return HttpService:RequestAsync({
			Url = SERVER_URL .. "/bot/pending",
			Method = "GET",
			Headers = { ["Content-Type"] = "application/json" },
		})
	end)

	if ok and response.Success then
		local success, data = pcall(function()
			return HttpService:JSONDecode(response.Body)
		end)
		if success and data.command then
			-- Handle double-nested command structure from /bot/queue
			-- The queue wraps commands: {command: {command: {...actual...}}}
			local cmd = data.command
			if type(cmd) == "table" and cmd.command and type(cmd.command) == "table" then
				-- Unwrap the nested command
				cmd = cmd.command
			end
			return cmd, data.id
		end
	end
	return nil, nil
end

-- Report command result to HTTP server
local function reportResult(commandId, result, state)
	pcall(function()
		HttpService:RequestAsync({
			Url = SERVER_URL .. "/bot/result",
			Method = "POST",
			Headers = { ["Content-Type"] = "application/json" },
			Body = HttpService:JSONEncode({
				id = commandId,
				result = result,
				state = state,
				timestamp = os.clock(),
			}),
		})
	end)
end

-- Report state to HTTP server (heartbeat)
local function reportState()
	local state = getState()
	pcall(function()
		HttpService:RequestAsync({
			Url = SERVER_URL .. "/bot/state",
			Method = "POST",
			Headers = { ["Content-Type"] = "application/json" },
			Body = HttpService:JSONEncode(state),
		})
	end)
end

-- Send goodbye notification
local function sendGoodbye(reason)
	pcall(function()
		HttpService:RequestAsync({
			Url = SERVER_URL .. "/bot/lifecycle",
			Method = "POST",
			Headers = { ["Content-Type"] = "application/json" },
			Body = HttpService:JSONEncode({
				event = "goodbye",
				timestamp = os.clock(),
				reason = reason,
			}),
		})
	end)
	print("[BotRunner:Server] Sent goodbye:", reason)
end

-- =============================================================================
-- SERVER-SIDE COMMAND EXECUTION
-- Some commands (queryServer, waitForServer) run directly on server
-- =============================================================================

-- Serialize a value to JSON-safe format
local function serializeValue(value, depth)
	depth = depth or 0
	if depth > 5 then return "[max depth]" end

	local t = typeof(value)
	if t == "nil" then return nil
	elseif t == "boolean" or t == "number" or t == "string" then return value
	elseif t == "Vector3" then return {x = value.X, y = value.Y, z = value.Z, _type = "Vector3"}
	elseif t == "CFrame" then return {position = serializeValue(value.Position, depth+1), _type = "CFrame"}
	elseif t == "Color3" then return {r = value.R, g = value.G, b = value.B, _type = "Color3"}
	elseif t == "Instance" then return {name = value.Name, class = value.ClassName, path = value:GetFullName(), _type = "Instance"}
	elseif t == "table" then
		local result = {}
		for k, v in pairs(value) do
			result[tostring(k)] = serializeValue(v, depth + 1)
		end
		return result
	else
		return tostring(value)
	end
end

-- Handle commands that should execute on server (not relayed to client)
local function handleCommandOnServer(cmd, cmdId)
	local action = cmd.action or cmd.command

	-- queryServer: Execute Luau code on server and return result
	if action == "queryServer" then
		local code = cmd.code or (cmd.args and cmd.args.code)
		if not code then
			reportResult(cmdId, {
				success = false,
				error = "No code provided for queryServer",
				context = "server"
			}, getState())
			return true
		end

		print("[BotRunner:Server] Executing queryServer:", string.sub(code, 1, 50) .. "...")

		-- Try with "return" wrapper first for expressions
		local fn, loadErr = loadstring("return " .. code)
		if not fn then
			-- Try without wrapper for statements
			fn, loadErr = loadstring(code)
		end

		if fn then
			local success, result = pcall(fn)
			local serialized = serializeValue(result)
			reportResult(cmdId, {
				success = success,
				result = serialized,
				error = not success and tostring(result) or nil,
				context = "server"
			}, getState())
			print("[BotRunner:Server] queryServer completed, success:", success)
		else
			reportResult(cmdId, {
				success = false,
				error = "Failed to load code: " .. tostring(loadErr),
				context = "server"
			}, getState())
		end
		return true -- Handled on server
	end

	-- waitForServer: Poll a condition on server until true or timeout
	if action == "waitForServer" then
		local condition = cmd.condition or (cmd.args and cmd.args.condition)
		local timeout = cmd.timeout or (cmd.args and cmd.args.timeout) or 30
		local pollInterval = (cmd.pollInterval or (cmd.args and cmd.args.pollInterval) or 100) / 1000

		if not condition then
			reportResult(cmdId, {
				success = false,
				error = "No condition provided for waitForServer",
				context = "server"
			}, getState())
			return true
		end

		print("[BotRunner:Server] waitForServer starting, timeout:", timeout)

		-- Compile condition
		local condFn, loadErr = loadstring("return " .. condition)
		if not condFn then
			reportResult(cmdId, {
				success = false,
				error = "Failed to compile condition: " .. tostring(loadErr),
				context = "server"
			}, getState())
			return true
		end

		local startTime = os.clock()
		local conditionMet = false

		while os.clock() - startTime < timeout do
			local ok, result = pcall(condFn)
			if ok and result then
				conditionMet = true
				break
			end
			task.wait(pollInterval)
		end

		local elapsed = os.clock() - startTime
		reportResult(cmdId, {
			success = true,
			result = conditionMet,
			timedOut = not conditionMet,
			elapsed = elapsed,
			context = "server"
		}, getState())
		print("[BotRunner:Server] waitForServer completed, met:", conditionMet, "elapsed:", elapsed)
		return true -- Handled on server
	end

	-- waitForState: Wait for a specific condition in client-reported state
	-- Useful for waiting on animations, movement completion, game events
	if action == "waitForState" then
		local field = cmd.field or (cmd.args and cmd.args.field)
		local value = cmd.value or (cmd.args and cmd.args.value)
		local comparison = cmd.comparison or (cmd.args and cmd.args.comparison) or "equals"
		local timeout = cmd.timeout or (cmd.args and cmd.args.timeout) or 10
		local pollInterval = (cmd.pollInterval or (cmd.args and cmd.args.pollInterval) or 100) / 1000

		if not field then
			reportResult(cmdId, {
				success = false,
				error = "No field provided for waitForState",
				context = "server"
			}, getState())
			return true
		end

		print("[BotRunner:Server] waitForState starting, field:", field, "value:", value, "timeout:", timeout)

		local startTime = os.clock()
		local conditionMet = false
		local lastValue = nil

		while os.clock() - startTime < timeout do
			local state = getState()

			-- Navigate nested fields (e.g., "position.x")
			local currentValue = state
			for part in string.gmatch(field, "[^%.]+") do
				if type(currentValue) == "table" then
					currentValue = currentValue[part]
				else
					currentValue = nil
					break
				end
			end

			lastValue = currentValue

			-- Compare based on comparison type
			local matched = false
			if comparison == "equals" then
				matched = currentValue == value
			elseif comparison == "notEquals" then
				matched = currentValue ~= value
			elseif comparison == "greaterThan" and type(currentValue) == "number" and type(value) == "number" then
				matched = currentValue > value
			elseif comparison == "lessThan" and type(currentValue) == "number" and type(value) == "number" then
				matched = currentValue < value
			elseif comparison == "contains" and type(currentValue) == "string" and type(value) == "string" then
				matched = string.find(currentValue, value, 1, true) ~= nil
			elseif comparison == "exists" then
				matched = currentValue ~= nil
			elseif comparison == "truthy" then
				matched = currentValue and true or false
			elseif comparison == "changed" then
				-- Wait for value to be different from initial
				if lastValue == nil then
					lastValue = currentValue
				else
					matched = currentValue ~= lastValue
				end
			end

			if matched then
				conditionMet = true
				break
			end

			task.wait(pollInterval)
		end

		local elapsed = os.clock() - startTime
		reportResult(cmdId, {
			success = true,
			result = conditionMet,
			timedOut = not conditionMet,
			lastValue = serializeValue(lastValue),
			elapsed = elapsed,
			field = field,
			context = "server"
		}, getState())
		print("[BotRunner:Server] waitForState completed, met:", conditionMet, "lastValue:", lastValue, "elapsed:", elapsed)
		return true -- Handled on server
	end

	return false -- Not a server command, relay to client
end

-- Clean up when player leaves
Players.PlayerRemoving:Connect(function(player)
	playerStates[player] = nil
end)

-- Main polling loop
local lastStateReport = 0
print("[BotRunner:Server] Starting command polling loop...")

-- Wait for at least one player
while #Players:GetPlayers() == 0 do
	task.wait(0.1)
end

local running = true

-- Stop when all players leave
Players.PlayerRemoving:Connect(function()
	if #Players:GetPlayers() <= 1 then
		running = false
	end
end)

while running do
	-- Poll for commands
	local cmd, cmdId = pollForCommands()
	if cmd then
		print("[BotRunner:Server] Got command:", cmd.action or cmd.command)

		-- Check if this command should be handled on server
		local handledOnServer = handleCommandOnServer(cmd, cmdId)

		if not handledOnServer then
			-- Send command to all connected players
			for _, player in ipairs(Players:GetPlayers()) do
				botRemote:FireClient(player, {
					type = "command",
					id = cmdId,
					command = cmd,
				})
			end

			-- Wait briefly for result
			task.wait(0.2)

			-- Check for result and report
			local result = commandResults[cmdId]
			if result then
				reportResult(cmdId, result.result, result.state)
				commandResults[cmdId] = nil
				print("[BotRunner:Server] Command completed:", cmd.action or cmd.command)
			else
				-- Report with current state even if no explicit result
				reportResult(cmdId, { success = true, sent = true }, getState())
			end
		end
	end

	-- Periodic state report
	local now = os.clock()
	if now - lastStateReport >= STATE_REPORT_INTERVAL then
		reportState()
		lastStateReport = now
	end

	task.wait(POLL_INTERVAL)
end

sendGoodbye("players_left")
print("[BotRunner:Server] Stopped (no players)")
]]
