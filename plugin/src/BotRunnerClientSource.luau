-- ModuleScript that returns the BotRunner client source code
-- This is injected as a LocalScript into StarterPlayerScripts during playtest

return [[
-- BotRunnerClient
-- Client-side: Receives commands via RemoteEvent and controls the player character

local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")

print("[BotRunner:Client] Starting...")

local player = Players.LocalPlayer
if not player then
	warn("[BotRunner:Client] No LocalPlayer!")
	return
end

local character = player.Character or player.CharacterAdded:Wait()
local humanoid = character:WaitForChild("Humanoid", 10)
local rootPart = character:WaitForChild("HumanoidRootPart", 10)

if not humanoid or not rootPart then
	warn("[BotRunner:Client] Character not ready!")
	return
end

print("[BotRunner:Client] Player:", player.Name, "ready!")

-- Wait for RemoteEvent from server
local botRemote = ReplicatedStorage:WaitForChild("BotCommandRemote", 10)

if not botRemote then
	warn("[BotRunner:Client] BotCommandRemote not found!")
	return
end

-- Try to load BotController for advanced commands
local BotController = nil
local botControllerModule = ReplicatedStorage:FindFirstChild("BotController")
if botControllerModule then
	local success, result = pcall(function()
		return require(botControllerModule)
	end)
	if success then
		BotController = result
		print("[BotRunner:Client] BotController loaded!")
	end
end

-- Simple state function (fallback if BotController not available)
local function getSimpleState()
	local pos = rootPart.Position
	return {
		position = { x = pos.X, y = pos.Y, z = pos.Z },
		health = humanoid.Health,
		maxHealth = humanoid.MaxHealth,
		walkSpeed = humanoid.WalkSpeed,
	}
end

-- Get full state
local function getState()
	if BotController and BotController.getState then
		return BotController.getState()
	end
	return getSimpleState()
end

-- Execute command locally
local function executeCommand(cmd)
	local action = cmd.action or cmd.command
	print("[BotRunner:Client] Executing:", action)

	-- Try BotController first
	if BotController and BotController.executeCommand then
		local success, result = pcall(function()
			return BotController.executeCommand(cmd)
		end)
		if success then
			return result
		else
			warn("[BotRunner:Client] BotController error:", result)
		end
	end

	-- Fallback to simple commands
	if action == "jump" then
		humanoid.Jump = true
		print("[BotRunner:Client] Jumped!")
		return { success = true, action = "jump" }

	elseif action == "moveTo" then
		local pos = cmd.position or (cmd.args and cmd.args.position)
		if pos then
			local target = Vector3.new(pos.x or 0, pos.y or 0, pos.z or 0)
			humanoid:MoveTo(target)
			print("[BotRunner:Client] Moving to:", target)
			return { success = true, action = "moveTo", target = pos }
		end
		return { success = false, error = "No position provided" }

	elseif action == "moveToObject" then
		local objName = cmd.objectName or (cmd.args and cmd.args.objectName) or (cmd.args and cmd.args.name)
		if objName then
			local obj = workspace:FindFirstChild(objName, true)
			if obj then
				local target
				if obj:IsA("BasePart") then
					target = obj.Position
				elseif obj:IsA("Model") then
					target = obj:GetPivot().Position
				end
				if target then
					humanoid:MoveTo(target)
					print("[BotRunner:Client] Moving to object:", objName)
					return { success = true, action = "moveToObject", target = objName }
				end
			end
			return { success = false, error = "Object not found: " .. tostring(objName) }
		end
		return { success = false, error = "No object name provided" }

	elseif action == "state" or action == "getState" then
		return { success = true, state = getState() }

	elseif action == "equip" or action == "equipTool" then
		local toolName = cmd.name or (cmd.args and cmd.args.name)
		if toolName then
			local backpack = player:FindFirstChild("Backpack")
			if backpack then
				local tool = backpack:FindFirstChild(toolName)
				if tool and tool:IsA("Tool") then
					tool.Parent = character
					print("[BotRunner:Client] Equipped:", toolName)
					return { success = true, action = "equip", tool = toolName }
				end
			end
			return { success = false, error = "Tool not found: " .. tostring(toolName) }
		end
		return { success = false, error = "No tool name provided" }

	elseif action == "unequip" or action == "unequipTool" then
		local backpack = player:FindFirstChild("Backpack")
		for _, child in character:GetChildren() do
			if child:IsA("Tool") then
				child.Parent = backpack
				print("[BotRunner:Client] Unequipped:", child.Name)
				return { success = true, action = "unequip" }
			end
		end
		return { success = true, action = "unequip", note = "No tool equipped" }

	elseif action == "activate" or action == "activateTool" then
		for _, child in character:GetChildren() do
			if child:IsA("Tool") then
				child:Activate()
				print("[BotRunner:Client] Activated:", child.Name)
				return { success = true, action = "activate", tool = child.Name }
			end
		end
		return { success = false, error = "No tool equipped" }

	elseif action == "interact" then
		local objName = cmd.name or cmd.objectName or (cmd.args and (cmd.args.name or cmd.args.objectName))
		local searchRadius = 15

		for _, obj in workspace:GetDescendants() do
			if obj:IsA("ProximityPrompt") then
				local parent = obj.Parent
				if parent and parent:IsA("BasePart") then
					local dist = (parent.Position - rootPart.Position).Magnitude
					if dist <= searchRadius then
						if not objName or (parent.Name:lower():find(objName:lower()) or (parent.Parent and parent.Parent.Name:lower():find(objName:lower()))) then
							obj:InputHoldBegin()
							task.wait(obj.HoldDuration + 0.1)
							obj:InputHoldEnd()
							print("[BotRunner:Client] Interacted with:", parent.Name)
							return { success = true, action = "interact", target = parent.Name }
						end
					end
				end
			end
		end
		return { success = false, error = "No ProximityPrompt found nearby" }
	end

	return { success = false, error = "Unknown action: " .. tostring(action) }
end

-- Listen for commands from server
botRemote.OnClientEvent:Connect(function(data)
	if data.type == "command" then
		local result = executeCommand(data.command)
		local state = getState()

		-- Send result back to server
		botRemote:FireServer({
			type = "result",
			id = data.id,
			result = result,
			state = state,
		})

		print("[BotRunner:Client] Command completed:", data.command.action or data.command.command)
	end
end)

-- Periodically send state to server
task.spawn(function()
	while player and character and humanoid.Health > 0 do
		local state = getState()
		botRemote:FireServer({
			type = "state",
			state = state,
		})
		task.wait(0.5)
	end
end)

-- Handle character respawn
player.CharacterAdded:Connect(function(newChar)
	character = newChar
	humanoid = newChar:WaitForChild("Humanoid", 10)
	rootPart = newChar:WaitForChild("HumanoidRootPart", 10)
	print("[BotRunner:Client] Character respawned")
end)

print("[BotRunner:Client] Ready and listening for commands")

-- Auto movement test on spawn
task.spawn(function()
	task.wait(1) -- Wait for game to settle

	print("[MovementTest] Starting movement verification...")

	-- Record starting position
	local startPos = rootPart.Position
	print("[MovementTest] Start position:", startPos)

	-- Test 1: Jump
	print("[MovementTest] Testing jump...")
	humanoid.Jump = true
	task.wait(0.5)
	local afterJumpY = rootPart.Position.Y
	local jumpWorked = afterJumpY > startPos.Y + 0.5
	print("[MovementTest] Jump result:", jumpWorked and "SUCCESS" or "FAILED", "- Height:", afterJumpY - startPos.Y)

	-- Wait to land
	task.wait(1)

	-- Test 2: Walk forward
	print("[MovementTest] Testing movement...")
	local targetPos = rootPart.Position + Vector3.new(10, 0, 0)
	humanoid:MoveTo(targetPos)

	-- Wait for movement
	local moveStart = tick()
	local moved = false
	while tick() - moveStart < 3 do
		local dist = (rootPart.Position - startPos).Magnitude
		if dist > 2 then
			moved = true
			break
		end
		task.wait(0.1)
	end

	local endPos = rootPart.Position
	local distMoved = (endPos - startPos).Magnitude
	print("[MovementTest] Movement result:", moved and "SUCCESS" or "FAILED", "- Distance:", string.format("%.1f", distMoved), "studs")

	-- Test 3: Jump while moving
	print("[MovementTest] Testing jump while moving...")
	humanoid.Jump = true
	task.wait(0.3)
	local midJumpY = rootPart.Position.Y
	print("[MovementTest] Mid-jump height:", string.format("%.1f", midJumpY - endPos.Y))

	-- Summary
	task.wait(1)
	print("[MovementTest] === SUMMARY ===")
	print("[MovementTest] Jump:", jumpWorked and "PASS" or "FAIL")
	print("[MovementTest] Walk:", moved and "PASS" or "FAIL")
	print("[MovementTest] Total distance moved:", string.format("%.1f", (rootPart.Position - startPos).Magnitude), "studs")
	print("[MovementTest] === END ===")
end)
]]
