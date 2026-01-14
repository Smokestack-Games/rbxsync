--!strict
-- BotController.luau
-- AI-powered character control for automated gameplay testing
-- Enables goal-based testing: navigation, combat, economy, UI interactions

local BotController = {}

-- Services
local PathfindingService = game:GetService("PathfindingService")
local Players = game:GetService("Players")
local RunService = game:GetService("RunService")
local CollectionService = game:GetService("CollectionService")
local ProximityPromptService = game:GetService("ProximityPromptService")
local UserInputService = game:GetService("UserInputService")
local TweenService = game:GetService("TweenService")

-- Types
export type Vector3Like = {x: number, y: number, z: number}

export type MoveResult = {
	success: boolean,
	reached: boolean,
	finalPosition: Vector3?,
	error: string?,
	pathLength: number?,
}

export type ActionResult = {
	success: boolean,
	result: any?,
	error: string?,
}

export type ObjectInfo = {
	name: string,
	className: string,
	position: Vector3?,
	distance: number?,
	path: string,
	tags: {string}?,
}

export type GameState = {
	position: Vector3?,
	health: number?,
	maxHealth: number?,
	walkSpeed: number?,
	jumpPower: number?,
	inventory: {string},
	equipped: string?,
	nearbyObjects: {ObjectInfo},
	nearbyNPCs: {ObjectInfo},
	nearbyPlayers: {ObjectInfo},
	visibleUI: {string},
	isMoving: boolean,
	isJumping: boolean,
	currentAction: string?,
	-- State tracking for test assertions
	reachedTargets: {[string]: number}?,
	sessionDuration: number?,
}

-- Configuration
local config = {
	-- Pathfinding
	agentRadius = 2,
	agentHeight = 5,
	agentCanJump = true,
	agentCanClimb = false,
	waypointSpacing = 4,

	-- Movement
	movementTimeout = 30, -- seconds
	stuckThreshold = 0.5, -- studs per second considered stuck
	stuckCheckInterval = 2, -- seconds
	recomputePathOnBlocked = true,

	-- Observation
	nearbyRadius = 50, -- studs
	maxNearbyObjects = 50,

	-- Actions
	interactionRange = 10, -- studs
}

-- Internal state
local currentPath: Path? = nil
local isMoving = false
local movementConnection: RBXScriptConnection? = nil
local currentAction: string? = nil
local lastPosition: Vector3? = nil
local stuckCheckTime: number = 0

-- State tracking for test assertions
local stateTracker = {
	reachedTargets = {} :: {[string]: number}, -- targetName -> timestamp
	positionHistory = {} :: {{position: Vector3, timestamp: number}},
	actionHistory = {} :: {{action: string, result: boolean, timestamp: number}},
	startTime = tick(),
}
local MAX_POSITION_HISTORY = 100

-- Record that we reached a target (called from movement functions)
local function recordReachedTarget(targetName: string)
	stateTracker.reachedTargets[targetName] = tick()
end

-- Record position to history
local function recordPosition(position: Vector3)
	table.insert(stateTracker.positionHistory, {
		position = position,
		timestamp = tick(),
	})
	-- Keep history bounded
	while #stateTracker.positionHistory > MAX_POSITION_HISTORY do
		table.remove(stateTracker.positionHistory, 1)
	end
end

-- Record action to history
local function recordAction(action: string, success: boolean)
	table.insert(stateTracker.actionHistory, {
		action = action,
		result = success,
		timestamp = tick(),
	})
	-- Keep last 50 actions
	while #stateTracker.actionHistory > 50 do
		table.remove(stateTracker.actionHistory, 1)
	end
end

-- =============================================================================
-- UTILITY FUNCTIONS
-- =============================================================================

local function getLocalPlayer(): Player?
	-- During playtest, get the local player
	return Players.LocalPlayer
end

local function getCharacter(): Model?
	local player = getLocalPlayer()
	return player and player.Character
end

local function getHumanoid(): Humanoid?
	local character = getCharacter()
	return character and character:FindFirstChildOfClass("Humanoid")
end

local function getHumanoidRootPart(): BasePart?
	local character = getCharacter()
	return character and character:FindFirstChild("HumanoidRootPart") :: BasePart?
end

local function vector3FromTable(v: Vector3Like): Vector3
	return Vector3.new(v.x, v.y, v.z)
end

local function getInstancePath(instance: Instance): string
	local path = instance.Name
	local current = instance.Parent
	while current and current ~= game do
		path = current.Name .. "." .. path
		current = current.Parent
	end
	return path
end

local function findInstanceByPath(path: string): Instance?
	local parts = string.split(path, ".")
	local current: Instance = game

	for _, part in ipairs(parts) do
		local child = current:FindFirstChild(part)
		if not child then
			return nil
		end
		current = child
	end

	return current
end

-- =============================================================================
-- MOVEMENT SYSTEM
-- =============================================================================

-- Create a path to target position
local function createPath(): Path
	return PathfindingService:CreatePath({
		AgentRadius = config.agentRadius,
		AgentHeight = config.agentHeight,
		AgentCanJump = config.agentCanJump,
		AgentCanClimb = config.agentCanClimb,
		WaypointSpacing = config.waypointSpacing,
	})
end

-- Move to a specific position using pathfinding
function BotController.moveTo(target: Vector3Like): MoveResult
	local targetPos = vector3FromTable(target)
	local humanoid = getHumanoid()
	local rootPart = getHumanoidRootPart()

	if not humanoid or not rootPart then
		return {
			success = false,
			reached = false,
			error = "No character or humanoid found",
		}
	end

	-- Stop any current movement
	BotController.stopMovement()

	isMoving = true
	currentAction = "moving"
	local startTime = os.clock()
	local startPosition = rootPart.Position

	-- Create and compute path
	local path = createPath()
	local computeSuccess, computeError = pcall(function()
		path:ComputeAsync(rootPart.Position, targetPos)
	end)

	if not computeSuccess then
		isMoving = false
		currentAction = nil
		return {
			success = false,
			reached = false,
			error = "Failed to compute path: " .. tostring(computeError),
		}
	end

	if path.Status ~= Enum.PathStatus.Success then
		isMoving = false
		currentAction = nil
		return {
			success = false,
			reached = false,
			error = "Path computation failed: " .. tostring(path.Status),
		}
	end

	local waypoints = path:GetWaypoints()
	if #waypoints == 0 then
		isMoving = false
		currentAction = nil
		return {
			success = false,
			reached = false,
			error = "No waypoints in path",
		}
	end

	-- Calculate path length
	local pathLength = 0
	for i = 2, #waypoints do
		pathLength = pathLength + (waypoints[i].Position - waypoints[i-1].Position).Magnitude
	end

	-- Follow the path
	local currentWaypointIndex = 2 -- Start at 2, first waypoint is current position
	local reachedDestination = false
	local blocked = false

	-- Handle path blocked
	local blockedConnection = path.Blocked:Connect(function(blockedIndex)
		if blockedIndex >= currentWaypointIndex and config.recomputePathOnBlocked then
			blocked = true
		end
	end)

	-- Move through waypoints
	while currentWaypointIndex <= #waypoints and isMoving do
		local waypoint = waypoints[currentWaypointIndex]

		-- Handle jump waypoints
		if waypoint.Action == Enum.PathWaypointAction.Jump then
			humanoid.Jump = true
		end

		-- Move to waypoint
		humanoid:MoveTo(waypoint.Position)

		-- Wait for movement to finish or timeout
		local moveFinished = false
		local moveConnection = humanoid.MoveToFinished:Connect(function(reached)
			moveFinished = true
		end)

		local waypointStartTime = os.clock()
		while not moveFinished and isMoving do
			-- Check for timeout
			if os.clock() - startTime > config.movementTimeout then
				moveConnection:Disconnect()
				break
			end

			-- Check for stuck
			if os.clock() - waypointStartTime > 3 then
				local currentPos = rootPart.Position
				local distanceToWaypoint = (currentPos - waypoint.Position).Magnitude
				if distanceToWaypoint < 3 then
					-- Close enough, move to next waypoint
					moveFinished = true
				end
			end

			task.wait(0.1)
		end

		moveConnection:Disconnect()

		-- Check if path was blocked
		if blocked then
			blocked = false
			-- Recompute path from current position
			local recomputeSuccess = pcall(function()
				path:ComputeAsync(rootPart.Position, targetPos)
			end)
			if recomputeSuccess and path.Status == Enum.PathStatus.Success then
				waypoints = path:GetWaypoints()
				currentWaypointIndex = 2
			else
				break
			end
		else
			currentWaypointIndex = currentWaypointIndex + 1
		end

		-- Check timeout
		if os.clock() - startTime > config.movementTimeout then
			break
		end
	end

	blockedConnection:Disconnect()

	-- Check if we reached destination
	local finalPosition = rootPart.Position
	local distanceToTarget = (finalPosition - targetPos).Magnitude
	reachedDestination = distanceToTarget < 5 -- Within 5 studs

	isMoving = false
	currentAction = nil

	return {
		success = true,
		reached = reachedDestination,
		finalPosition = finalPosition,
		pathLength = pathLength,
		error = not reachedDestination and ("Stopped " .. string.format("%.1f", distanceToTarget) .. " studs from target") or nil,
	}
end

-- Move to a named object
function BotController.moveToObject(objectName: string): MoveResult
	-- Find the object
	local function findObject(parent: Instance): Instance?
		for _, child in ipairs(parent:GetDescendants()) do
			if child.Name == objectName then
				return child
			end
		end
		return nil
	end

	local target = findObject(workspace)
	if not target then
		return {
			success = false,
			reached = false,
			error = "Object not found: " .. objectName,
		}
	end

	-- Get position
	local position: Vector3
	if target:IsA("BasePart") then
		position = target.Position
	elseif target:IsA("Model") then
		local primaryPart = target.PrimaryPart or target:FindFirstChildWhichIsA("BasePart")
		if primaryPart then
			position = primaryPart.Position
		else
			return {
				success = false,
				reached = false,
				error = "Object has no position: " .. objectName,
			}
		end
	else
		return {
			success = false,
			reached = false,
			error = "Object type not supported for movement: " .. target.ClassName,
		}
	end

	local result = BotController.moveTo({x = position.X, y = position.Y, z = position.Z})

	-- Record reached target if successful
	if result.success and result.reached then
		recordReachedTarget(objectName)
	end

	return result
end

-- Stop current movement
function BotController.stopMovement()
	isMoving = false
	local humanoid = getHumanoid()
	if humanoid then
		humanoid:MoveTo(getHumanoidRootPart().Position) -- Stop at current position
	end
	currentAction = nil
end

-- Simple direct movement (no pathfinding)
function BotController.walkTo(target: Vector3Like): MoveResult
	local targetPos = vector3FromTable(target)
	local humanoid = getHumanoid()
	local rootPart = getHumanoidRootPart()

	if not humanoid or not rootPart then
		return {
			success = false,
			reached = false,
			error = "No character or humanoid found",
		}
	end

	isMoving = true
	currentAction = "walking"

	humanoid:MoveTo(targetPos)

	local reached = humanoid.MoveToFinished:Wait()

	isMoving = false
	currentAction = nil

	return {
		success = true,
		reached = reached,
		finalPosition = rootPart.Position,
	}
end

-- Jump
function BotController.jump(): ActionResult
	local humanoid = getHumanoid()
	if not humanoid then
		return { success = false, error = "No humanoid found" }
	end

	humanoid.Jump = true
	return { success = true }
end

-- =============================================================================
-- ACTION SYSTEM
-- =============================================================================

-- Equip a tool from backpack
function BotController.equipTool(toolName: string): ActionResult
	local player = getLocalPlayer()
	if not player then
		return { success = false, error = "No player found" }
	end

	local backpack = player:FindFirstChild("Backpack")
	local character = getCharacter()

	if not backpack or not character then
		return { success = false, error = "No backpack or character" }
	end

	-- First unequip current tool
	for _, child in ipairs(character:GetChildren()) do
		if child:IsA("Tool") then
			child.Parent = backpack
		end
	end

	-- Find and equip the tool
	local tool = backpack:FindFirstChild(toolName)
	if not tool or not tool:IsA("Tool") then
		return { success = false, error = "Tool not found: " .. toolName }
	end

	tool.Parent = character
	return { success = true, result = toolName }
end

-- Unequip current tool
function BotController.unequipTool(): ActionResult
	local player = getLocalPlayer()
	if not player then
		return { success = false, error = "No player found" }
	end

	local backpack = player:FindFirstChild("Backpack")
	local character = getCharacter()

	if not backpack or not character then
		return { success = false, error = "No backpack or character" }
	end

	local unequipped = false
	for _, child in ipairs(character:GetChildren()) do
		if child:IsA("Tool") then
			child.Parent = backpack
			unequipped = true
		end
	end

	return { success = true, result = unequipped }
end

-- Get currently equipped tool
function BotController.getEquippedTool(): string?
	local character = getCharacter()
	if not character then return nil end

	for _, child in ipairs(character:GetChildren()) do
		if child:IsA("Tool") then
			return child.Name
		end
	end
	return nil
end

-- Activate equipped tool
function BotController.activateTool(): ActionResult
	local character = getCharacter()
	if not character then
		return { success = false, error = "No character found" }
	end

	local tool = character:FindFirstChildWhichIsA("Tool")
	if not tool then
		return { success = false, error = "No tool equipped" }
	end

	tool:Activate()
	return { success = true, result = tool.Name }
end

-- Deactivate equipped tool
function BotController.deactivateTool(): ActionResult
	local character = getCharacter()
	if not character then
		return { success = false, error = "No character found" }
	end

	local tool = character:FindFirstChildWhichIsA("Tool")
	if not tool then
		return { success = false, error = "No tool equipped" }
	end

	tool:Deactivate()
	return { success = true, result = tool.Name }
end

-- Interact with nearby ProximityPrompt
function BotController.interact(objectName: string?): ActionResult
	local rootPart = getHumanoidRootPart()
	if not rootPart then
		return { success = false, error = "No character found" }
	end

	local nearestPrompt: ProximityPrompt? = nil
	local nearestDistance = config.interactionRange

	-- Find proximity prompts
	for _, descendant in ipairs(workspace:GetDescendants()) do
		if descendant:IsA("ProximityPrompt") then
			local promptParent = descendant.Parent
			if promptParent and promptParent:IsA("BasePart") then
				local distance = (promptParent.Position - rootPart.Position).Magnitude

				-- Check if matches object name filter
				local matches = true
				if objectName then
					matches = promptParent.Name == objectName or
							  (promptParent.Parent and promptParent.Parent.Name == objectName)
				end

				if matches and distance < nearestDistance then
					nearestDistance = distance
					nearestPrompt = descendant
				end
			end
		end
	end

	if not nearestPrompt then
		return { success = false, error = "No interaction prompt found" .. (objectName and " for " .. objectName or "") }
	end

	-- Trigger the prompt
	local player = getLocalPlayer()
	if player then
		-- Fire the prompt
		nearestPrompt:InputHoldBegin()
		task.wait(nearestPrompt.HoldDuration + 0.1)
		nearestPrompt:InputHoldEnd()
	end

	return { success = true, result = nearestPrompt.ObjectText or nearestPrompt.ActionText }
end

-- Fire a remote event (for game-specific abilities)
function BotController.fireRemote(remotePath: string, ...: any): ActionResult
	local remote = findInstanceByPath(remotePath)
	if not remote then
		return { success = false, error = "Remote not found: " .. remotePath }
	end

	if remote:IsA("RemoteEvent") then
		remote:FireServer(...)
		return { success = true }
	elseif remote:IsA("RemoteFunction") then
		local result = remote:InvokeServer(...)
		return { success = true, result = result }
	else
		return { success = false, error = "Not a remote: " .. remote.ClassName }
	end
end

-- Drop a pachinko ball (game-specific action)
function BotController.dropBall(dropPosition: number?, ballType: string?): ActionResult
	dropPosition = dropPosition or 0
	ballType = ballType or "STANDARD"

	-- Find the DropBall remote event
	local ReplicatedStorage = game:GetService("ReplicatedStorage")
	local remotes = ReplicatedStorage:FindFirstChild("Remotes")
	local dropBallRemote = remotes and remotes:FindFirstChild("DropBall")

	if not dropBallRemote then
		-- Try alternative paths
		dropBallRemote = ReplicatedStorage:FindFirstChild("DropBall")
	end

	if not dropBallRemote then
		return { success = false, error = "DropBall remote not found" }
	end

	-- Fire the remote
	if dropBallRemote:IsA("RemoteEvent") then
		dropBallRemote:FireServer({
			dropPosition = dropPosition,
			ballType = ballType,
		})
		return { success = true, result = { dropPosition = dropPosition, ballType = ballType } }
	elseif dropBallRemote:IsA("RemoteFunction") then
		local result = dropBallRemote:InvokeServer({
			dropPosition = dropPosition,
			ballType = ballType,
		})
		return { success = true, result = result }
	else
		return { success = false, error = "DropBall is not a remote: " .. dropBallRemote.ClassName }
	end
end

-- Click a ClickDetector
function BotController.clickObject(objectName: string): ActionResult
	local rootPart = getHumanoidRootPart()
	local player = getLocalPlayer()
	if not rootPart or not player then
		return { success = false, error = "No character found" }
	end

	-- Find click detector
	for _, descendant in ipairs(workspace:GetDescendants()) do
		if descendant:IsA("ClickDetector") then
			local parent = descendant.Parent
			if parent and (parent.Name == objectName or (parent.Parent and parent.Parent.Name == objectName)) then
				local parentPart = parent:IsA("BasePart") and parent or parent:FindFirstChildWhichIsA("BasePart")
				if parentPart then
					local distance = (parentPart.Position - rootPart.Position).Magnitude
					if distance <= descendant.MaxActivationDistance then
						-- Simulate click
						-- Note: ClickDetector doesn't have a direct fire method, but games often use MouseClick event
						-- We can try to find and fire it
						local clickEvent = descendant:FindFirstChild("MouseClick")
						if clickEvent and clickEvent:IsA("BindableEvent") then
							clickEvent:Fire(player)
							return { success = true, result = objectName }
						end
						return { success = false, error = "Cannot programmatically click - ClickDetector requires actual mouse input" }
					end
				end
			end
		end
	end

	return { success = false, error = "Click detector not found: " .. objectName }
end

-- =============================================================================
-- UI INTERACTION SYSTEM
-- =============================================================================

-- Find GUI element by path
function BotController.findUI(path: string): Instance?
	local player = getLocalPlayer()
	if not player then return nil end

	local playerGui = player:FindFirstChild("PlayerGui")
	if not playerGui then return nil end

	-- If path starts with "PlayerGui.", remove it
	if string.sub(path, 1, 10) == "PlayerGui." then
		path = string.sub(path, 11)
	end

	local parts = string.split(path, ".")
	local current: Instance = playerGui

	for _, part in ipairs(parts) do
		local child = current:FindFirstChild(part)
		if not child then
			return nil
		end
		current = child
	end

	return current
end

-- Click a GUI button
function BotController.clickButton(buttonPath: string): ActionResult
	local button = BotController.findUI(buttonPath)
	if not button then
		return { success = false, error = "Button not found: " .. buttonPath }
	end

	if button:IsA("GuiButton") then
		-- Fire the Activated event
		-- Note: We can't directly fire Activated, but we can trigger MouseButton1Click
		local connection
		local clicked = false

		-- Check for any connected functions
		if button:IsA("TextButton") or button:IsA("ImageButton") then
			-- Simulate by directly triggering any BindableEvents or finding Click handlers
			-- Most games use MouseButton1Click which we can't fire directly
			-- But we can try Activated event
			button.Activated:Fire()
			return { success = true, result = buttonPath }
		end
	end

	return { success = false, error = "Not a clickable button: " .. buttonPath }
end

-- Read text from a GUI element
function BotController.readText(labelPath: string): ActionResult
	local label = BotController.findUI(labelPath)
	if not label then
		return { success = false, error = "Label not found: " .. labelPath }
	end

	if label:IsA("TextLabel") or label:IsA("TextButton") or label:IsA("TextBox") then
		return { success = true, result = label.Text }
	end

	return { success = false, error = "Element has no text: " .. labelPath }
end

-- Fill a text box
function BotController.fillTextBox(textBoxPath: string, text: string): ActionResult
	local textBox = BotController.findUI(textBoxPath)
	if not textBox then
		return { success = false, error = "TextBox not found: " .. textBoxPath }
	end

	if textBox:IsA("TextBox") then
		textBox.Text = text
		-- Fire FocusLost to simulate user finishing input
		task.wait(0.1)
		return { success = true, result = text }
	end

	return { success = false, error = "Not a TextBox: " .. textBoxPath }
end

-- Get all visible UI screens
function BotController.getVisibleUI(): {string}
	local player = getLocalPlayer()
	if not player then return {} end

	local playerGui = player:FindFirstChild("PlayerGui")
	if not playerGui then return {} end

	local visible = {}

	for _, screenGui in ipairs(playerGui:GetChildren()) do
		if screenGui:IsA("ScreenGui") and screenGui.Enabled then
			-- Check if any frames/buttons are visible
			local hasVisibleContent = false
			for _, child in ipairs(screenGui:GetDescendants()) do
				if child:IsA("GuiObject") and child.Visible then
					hasVisibleContent = true
					break
				end
			end
			if hasVisibleContent then
				table.insert(visible, screenGui.Name)
			end
		end
	end

	return visible
end

-- =============================================================================
-- OBSERVATION SYSTEM
-- =============================================================================

-- Get nearby objects
function BotController.getNearbyObjects(radius: number?): {ObjectInfo}
	local searchRadius = radius or config.nearbyRadius
	local rootPart = getHumanoidRootPart()
	if not rootPart then return {} end

	local objects: {ObjectInfo} = {}
	local position = rootPart.Position

	for _, descendant in ipairs(workspace:GetDescendants()) do
		if descendant:IsA("BasePart") and descendant ~= rootPart then
			local distance = (descendant.Position - position).Magnitude
			if distance <= searchRadius then
				-- Skip character parts
				local isCharacterPart = false
				local parent = descendant.Parent
				while parent do
					if parent:FindFirstChildOfClass("Humanoid") then
						isCharacterPart = true
						break
					end
					parent = parent.Parent
				end

				if not isCharacterPart then
					table.insert(objects, {
						name = descendant.Name,
						className = descendant.ClassName,
						position = descendant.Position,
						distance = distance,
						path = getInstancePath(descendant),
						tags = CollectionService:GetTags(descendant),
					})
				end
			end
		end
	end

	-- Sort by distance
	table.sort(objects, function(a, b) return (a.distance or 0) < (b.distance or 0) end)

	-- Limit results
	if #objects > config.maxNearbyObjects then
		local limited = {}
		for i = 1, config.maxNearbyObjects do
			limited[i] = objects[i]
		end
		return limited
	end

	return objects
end

-- Get nearby NPCs (models with humanoids that aren't players)
function BotController.getNearbyNPCs(radius: number?): {ObjectInfo}
	local searchRadius = radius or config.nearbyRadius
	local rootPart = getHumanoidRootPart()
	if not rootPart then return {} end

	local npcs: {ObjectInfo} = {}
	local position = rootPart.Position
	local playerCharacters = {}

	-- Build set of player characters
	for _, player in ipairs(Players:GetPlayers()) do
		if player.Character then
			playerCharacters[player.Character] = true
		end
	end

	for _, descendant in ipairs(workspace:GetDescendants()) do
		if descendant:IsA("Humanoid") and not playerCharacters[descendant.Parent] then
			local model = descendant.Parent
			if model and model:IsA("Model") then
				local npcRoot = model:FindFirstChild("HumanoidRootPart") or model.PrimaryPart
				if npcRoot and npcRoot:IsA("BasePart") then
					local distance = (npcRoot.Position - position).Magnitude
					if distance <= searchRadius then
						table.insert(npcs, {
							name = model.Name,
							className = "NPC",
							position = npcRoot.Position,
							distance = distance,
							path = getInstancePath(model),
							tags = CollectionService:GetTags(model),
						})
					end
				end
			end
		end
	end

	-- Sort by distance
	table.sort(npcs, function(a, b) return (a.distance or 0) < (b.distance or 0) end)

	return npcs
end

-- Get nearby players (other than local player)
function BotController.getNearbyPlayers(radius: number?): {ObjectInfo}
	local searchRadius = radius or config.nearbyRadius
	local rootPart = getHumanoidRootPart()
	local localPlayer = getLocalPlayer()
	if not rootPart or not localPlayer then return {} end

	local players: {ObjectInfo} = {}
	local position = rootPart.Position

	for _, player in ipairs(Players:GetPlayers()) do
		if player ~= localPlayer and player.Character then
			local playerRoot = player.Character:FindFirstChild("HumanoidRootPart")
			if playerRoot and playerRoot:IsA("BasePart") then
				local distance = (playerRoot.Position - position).Magnitude
				if distance <= searchRadius then
					local humanoid = player.Character:FindFirstChildOfClass("Humanoid")
					table.insert(players, {
						name = player.Name,
						className = "Player",
						position = playerRoot.Position,
						distance = distance,
						path = "Players." .. player.Name,
						tags = {},
						health = humanoid and humanoid.Health,
						maxHealth = humanoid and humanoid.MaxHealth,
					})
				end
			end
		end
	end

	-- Sort by distance
	table.sort(players, function(a, b) return (a.distance or 0) < (b.distance or 0) end)

	return players
end

-- Get player inventory
function BotController.getInventory(): {string}
	local player = getLocalPlayer()
	if not player then return {} end

	local backpack = player:FindFirstChild("Backpack")
	local character = getCharacter()

	local items = {}

	if backpack then
		for _, item in ipairs(backpack:GetChildren()) do
			if item:IsA("Tool") then
				table.insert(items, item.Name)
			end
		end
	end

	if character then
		for _, item in ipairs(character:GetChildren()) do
			if item:IsA("Tool") then
				table.insert(items, item.Name .. " (equipped)")
			end
		end
	end

	return items
end

-- Get complete game state snapshot
function BotController.getState(): GameState
	local humanoid = getHumanoid()
	local rootPart = getHumanoidRootPart()

	-- Record current position to history
	if rootPart then
		recordPosition(rootPart.Position)
	end

	return {
		position = rootPart and rootPart.Position,
		health = humanoid and humanoid.Health,
		maxHealth = humanoid and humanoid.MaxHealth,
		walkSpeed = humanoid and humanoid.WalkSpeed,
		jumpPower = humanoid and humanoid.JumpPower,
		inventory = BotController.getInventory(),
		equipped = BotController.getEquippedTool(),
		nearbyObjects = BotController.getNearbyObjects(30), -- Smaller radius for state
		nearbyNPCs = BotController.getNearbyNPCs(50),
		nearbyPlayers = BotController.getNearbyPlayers(50),
		visibleUI = BotController.getVisibleUI(),
		isMoving = isMoving,
		isJumping = humanoid and humanoid.Jump or false,
		currentAction = currentAction,
		-- State tracking for test assertions
		reachedTargets = stateTracker.reachedTargets,
		sessionDuration = tick() - stateTracker.startTime,
	}
end

-- Find objects by name pattern
function BotController.findObjects(query: string): {ObjectInfo}
	local rootPart = getHumanoidRootPart()
	local position = rootPart and rootPart.Position or Vector3.new(0, 0, 0)

	local results: {ObjectInfo} = {}
	local queryLower = string.lower(query)

	for _, descendant in ipairs(workspace:GetDescendants()) do
		if string.find(string.lower(descendant.Name), queryLower) then
			local objPosition: Vector3? = nil
			local distance: number? = nil

			if descendant:IsA("BasePart") then
				objPosition = descendant.Position
			elseif descendant:IsA("Model") then
				local primary = descendant.PrimaryPart or descendant:FindFirstChildWhichIsA("BasePart")
				if primary then
					objPosition = primary.Position
				end
			end

			if objPosition and rootPart then
				distance = (objPosition - position).Magnitude
			end

			table.insert(results, {
				name = descendant.Name,
				className = descendant.ClassName,
				position = objPosition,
				distance = distance,
				path = getInstancePath(descendant),
				tags = CollectionService:GetTags(descendant),
			})
		end
	end

	-- Sort by distance if available
	table.sort(results, function(a, b)
		if a.distance and b.distance then
			return a.distance < b.distance
		end
		return a.name < b.name
	end)

	return results
end

-- =============================================================================
-- COMMAND DISPATCHER
-- =============================================================================

export type Command = {
	type: string,
	command: string,
	args: {[string]: any}?,
}

export type CommandResult = {
	success: boolean,
	result: any?,
	error: string?,
	state: GameState?,
}

-- Execute a command from external source
function BotController.executeCommand(cmd: Command): CommandResult
	local commandType = cmd.type
	local commandName = cmd.command
	local args = cmd.args or {}

	local success, result, error_msg

	-- Movement commands
	if commandType == "move" then
		if commandName == "moveTo" then
			local moveResult = BotController.moveTo(args.position or args)
			success = moveResult.success
			result = moveResult
			error_msg = moveResult.error
		elseif commandName == "moveToObject" then
			local moveResult = BotController.moveToObject(args.name or args.objectName)
			success = moveResult.success
			result = moveResult
			error_msg = moveResult.error
		elseif commandName == "walkTo" then
			local moveResult = BotController.walkTo(args.position or args)
			success = moveResult.success
			result = moveResult
			error_msg = moveResult.error
		elseif commandName == "stop" then
			BotController.stopMovement()
			success = true
		elseif commandName == "jump" then
			local actionResult = BotController.jump()
			success = actionResult.success
			error_msg = actionResult.error
		else
			success = false
			error_msg = "Unknown move command: " .. commandName
		end

	-- Action commands
	elseif commandType == "action" then
		if commandName == "equipTool" then
			local actionResult = BotController.equipTool(args.name or args.toolName)
			success = actionResult.success
			result = actionResult.result
			error_msg = actionResult.error
		elseif commandName == "unequipTool" then
			local actionResult = BotController.unequipTool()
			success = actionResult.success
			result = actionResult.result
			error_msg = actionResult.error
		elseif commandName == "activateTool" then
			local actionResult = BotController.activateTool()
			success = actionResult.success
			result = actionResult.result
			error_msg = actionResult.error
		elseif commandName == "deactivateTool" then
			local actionResult = BotController.deactivateTool()
			success = actionResult.success
			result = actionResult.result
			error_msg = actionResult.error
		elseif commandName == "interact" then
			local actionResult = BotController.interact(args.name or args.objectName)
			success = actionResult.success
			result = actionResult.result
			error_msg = actionResult.error
		elseif commandName == "fireRemote" then
			local actionResult = BotController.fireRemote(args.path, table.unpack(args.args or {}))
			success = actionResult.success
			result = actionResult.result
			error_msg = actionResult.error
		elseif commandName == "dropBall" then
			local actionResult = BotController.dropBall(args.x or args.dropPosition, args.ballType)
			success = actionResult.success
			result = actionResult.result
			error_msg = actionResult.error
		else
			success = false
			error_msg = "Unknown action command: " .. commandName
		end

	-- UI commands
	elseif commandType == "ui" then
		if commandName == "clickButton" then
			local actionResult = BotController.clickButton(args.path or args.buttonPath)
			success = actionResult.success
			result = actionResult.result
			error_msg = actionResult.error
		elseif commandName == "readText" then
			local actionResult = BotController.readText(args.path or args.labelPath)
			success = actionResult.success
			result = actionResult.result
			error_msg = actionResult.error
		elseif commandName == "fillTextBox" then
			local actionResult = BotController.fillTextBox(args.path or args.textBoxPath, args.text)
			success = actionResult.success
			result = actionResult.result
			error_msg = actionResult.error
		elseif commandName == "getVisibleUI" then
			result = BotController.getVisibleUI()
			success = true
		else
			success = false
			error_msg = "Unknown UI command: " .. commandName
		end

	-- Observation commands
	elseif commandType == "observe" then
		if commandName == "getState" then
			result = BotController.getState()
			success = true
		elseif commandName == "getNearbyObjects" then
			result = BotController.getNearbyObjects(args.radius)
			success = true
		elseif commandName == "getNearbyNPCs" then
			result = BotController.getNearbyNPCs(args.radius)
			success = true
		elseif commandName == "getInventory" then
			result = BotController.getInventory()
			success = true
		elseif commandName == "findObjects" then
			result = BotController.findObjects(args.query or args.name)
			success = true
		elseif commandName == "findUI" then
			local ui = BotController.findUI(args.path)
			result = ui and getInstancePath(ui)
			success = ui ~= nil
			error_msg = not ui and "UI not found: " .. (args.path or "")
		else
			success = false
			error_msg = "Unknown observe command: " .. commandName
		end
	else
		success = false
		error_msg = "Unknown command type: " .. tostring(commandType)
	end

	return {
		success = success,
		result = result,
		error = error_msg,
		state = BotController.getState(), -- Always include current state
	}
end

-- =============================================================================
-- CONFIGURATION
-- =============================================================================

function BotController.configure(newConfig: {[string]: any})
	for key, value in pairs(newConfig) do
		if config[key] ~= nil then
			config[key] = value
		end
	end
end

function BotController.getConfig(): {[string]: any}
	return table.clone(config)
end

return BotController
