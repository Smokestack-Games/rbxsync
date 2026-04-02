--!strict
-- Verifier.luau
-- E2E verification module for asserting against the live Roblox DataModel
-- Evaluates structured assertions for automated testing workflows

local Verifier = {}

-- Services
local CollectionService = game:GetService("CollectionService")
local Players = game:GetService("Players")

-- Known services for GetService resolution
local KNOWN_SERVICES: { [string]: boolean } = {
	Workspace = true,
	ServerScriptService = true,
	ServerStorage = true,
	ReplicatedStorage = true,
	StarterGui = true,
	StarterPack = true,
	StarterPlayer = true,
	Lighting = true,
	SoundService = true,
	Teams = true,
	Chat = true,
	TestService = true,
}

-- Types
export type CheckResult = {
	pass: boolean,
	message: string,
	actual: any?,
	expected: any?,
	elapsed: number?,
	error: string?,
}

export type CheckCommand = {
	type: string,
	path: string?,
	property: string?,
	attribute: string?,
	operator: string?,
	expected: any?,
	timeout: number?,
	tag: string?,
	class: string?,
	parent: string?,
	item: string?,
	stat: string?,
	pathA: string?,
	pathB: string?,
	name: string?,
}

--------------------------------------------------------------------------------
-- Path resolution
--------------------------------------------------------------------------------

local function resolvePath(path: string): Instance?
	local segments = string.split(path, "/")
	if #segments == 0 then
		return nil
	end

	local first = segments[1]
	local root: Instance?

	if KNOWN_SERVICES[first] then
		local ok, service = pcall(function()
			return game:GetService(first)
		end)
		if ok and service then
			root = service
		else
			return nil
		end
	else
		root = game:FindFirstChild(first)
	end

	if not root then
		return nil
	end

	for i = 2, #segments do
		root = (root :: Instance):FindFirstChild(segments[i])
		if not root then
			return nil
		end
	end

	return root
end

--------------------------------------------------------------------------------
-- Property reading with type conversion
--------------------------------------------------------------------------------

local function convertValue(value: any): any
	if typeof(value) == "Vector3" then
		return { X = value.X, Y = value.Y, Z = value.Z }
	elseif typeof(value) == "CFrame" then
		local pos = value.Position
		return { X = pos.X, Y = pos.Y, Z = pos.Z }
	elseif typeof(value) == "Color3" then
		return {
			R = math.floor(value.R * 255 + 0.5),
			G = math.floor(value.G * 255 + 0.5),
			B = math.floor(value.B * 255 + 0.5),
		}
	elseif typeof(value) == "EnumItem" then
		return value.Name
	else
		return value
	end
end

local function readProperty(instance: Instance, propertyPath: string): (boolean, any)
	local parts = string.split(propertyPath, ".")
	local current: any = instance

	local ok, err = pcall(function()
		for _, part in parts do
			current = current[part]
		end
	end)

	if not ok then
		return false, err
	end

	return true, convertValue(current)
end

--------------------------------------------------------------------------------
-- Operator comparison
--------------------------------------------------------------------------------

local function applyOperator(operator: string, actual: any, expected: any): boolean
	if operator == "eq" then
		return actual == expected
	elseif operator == "neq" then
		return actual ~= expected
	elseif operator == "gt" then
		local a = tonumber(actual)
		local b = tonumber(expected)
		return a ~= nil and b ~= nil and a > b
	elseif operator == "lt" then
		local a = tonumber(actual)
		local b = tonumber(expected)
		return a ~= nil and b ~= nil and a < b
	elseif operator == "contains" then
		if type(actual) == "string" and type(expected) == "string" then
			return string.find(actual, expected, 1, true) ~= nil
		elseif type(actual) == "table" then
			for _, v in actual do
				if v == expected then
					return true
				end
			end
			return false
		end
		return false
	elseif operator == "exists" then
		return actual ~= nil
	elseif operator == "not_exists" then
		return actual == nil
	end

	return false
end

--------------------------------------------------------------------------------
-- Player resolution
--------------------------------------------------------------------------------

local function getPlayer(): Player?
	local localPlayer = Players.LocalPlayer
	if localPlayer then
		return localPlayer
	end

	local allPlayers = Players:GetPlayers()
	if #allPlayers > 0 then
		return allPlayers[1]
	end

	return nil
end

--------------------------------------------------------------------------------
-- Check implementations
--------------------------------------------------------------------------------

local function executeCheck(cmd: CheckCommand): (boolean, any)
	local checkType = cmd.type

	if checkType == "property" then
		local instance = resolvePath(cmd.path :: string)
		if not instance then
			return false, "Instance not found: " .. tostring(cmd.path)
		end

		local ok, value = readProperty(instance, cmd.property :: string)
		if not ok then
			return false, "Property read failed: " .. tostring(value)
		end

		local pass = applyOperator(cmd.operator :: string, value, cmd.expected)
		return pass, value

	elseif checkType == "attribute" then
		local instance = resolvePath(cmd.path :: string)
		if not instance then
			return false, "Instance not found: " .. tostring(cmd.path)
		end

		local value = instance:GetAttribute(cmd.attribute :: string)
		local actual = convertValue(value)
		local pass = applyOperator(cmd.operator :: string, actual, cmd.expected)
		return pass, actual

	elseif checkType == "count" then
		local count = 0

		if cmd.tag then
			count = #CollectionService:GetTagged(cmd.tag)
		elseif cmd.class and cmd.parent then
			local parentInstance = resolvePath(cmd.parent)
			if not parentInstance then
				return false, "Parent not found: " .. tostring(cmd.parent)
			end
			for _, child in parentInstance:GetDescendants() do
				if child:IsA(cmd.class) then
					count += 1
				end
			end
		elseif cmd.class then
			local workspace = game:GetService("Workspace")
			for _, desc in workspace:GetDescendants() do
				if desc:IsA(cmd.class) then
					count += 1
				end
			end
		else
			return false, "count requires tag, class, or class+parent"
		end

		local pass = true
		if cmd.operator and cmd.expected then
			pass = applyOperator(cmd.operator, count, cmd.expected)
		end
		return pass, count

	elseif checkType == "find" then
		local instance = resolvePath(cmd.path :: string)
		local operator = cmd.operator or "exists"

		if operator == "exists" then
			return instance ~= nil, instance ~= nil
		elseif operator == "not_exists" then
			return instance == nil, instance == nil
		end

		return instance ~= nil, instance ~= nil

	elseif checkType == "children" then
		local instance = resolvePath(cmd.path :: string)
		if not instance then
			return false, "Instance not found: " .. tostring(cmd.path)
		end

		local children = instance:GetChildren()
		local result = {}

		for _, child in children do
			if cmd.class then
				if child:IsA(cmd.class) then
					table.insert(result, { name = child.Name, class = child.ClassName })
				end
			else
				table.insert(result, { name = child.Name, class = child.ClassName })
			end
		end

		return true, result

	elseif checkType == "distance" then
		local instA = resolvePath(cmd.pathA :: string)
		if not instA then
			return false, "Instance A not found: " .. tostring(cmd.pathA)
		end

		local instB = resolvePath(cmd.pathB :: string)
		if not instB then
			return false, "Instance B not found: " .. tostring(cmd.pathB)
		end

		local okA, posA = pcall(function()
			return (instA :: any).Position :: Vector3
		end)
		if not okA then
			return false, "Instance A has no Position property"
		end

		local okB, posB = pcall(function()
			return (instB :: any).Position :: Vector3
		end)
		if not okB then
			return false, "Instance B has no Position property"
		end

		local magnitude = (posA - posB).Magnitude

		if cmd.operator and cmd.expected then
			local pass = applyOperator(cmd.operator, magnitude, cmd.expected)
			return pass, magnitude
		end

		return true, magnitude

	elseif checkType == "backpack" then
		local player = getPlayer()
		if not player then
			return false, "No player found"
		end

		local item = cmd.item :: string
		local backpack = player:FindFirstChild("Backpack")
		local character = player.Character

		local inBackpack = backpack and backpack:FindFirstChild(item) ~= nil
		local equipped = character and character:FindFirstChild(item) ~= nil

		local hasItem = inBackpack or equipped
		local actual: any
		if equipped then
			actual = "equipped"
		elseif inBackpack then
			actual = "backpack"
		else
			actual = nil
		end

		local operator = cmd.operator or "exists"
		if operator == "exists" then
			return hasItem, actual
		elseif operator == "not_exists" then
			return not hasItem, actual
		elseif operator == "eq" then
			return actual == cmd.expected, actual
		end

		return hasItem, actual

	elseif checkType == "leaderstat" then
		local player = getPlayer()
		if not player then
			return false, "No player found"
		end

		local leaderstats = player:FindFirstChild("leaderstats")
		if not leaderstats then
			return false, "No leaderstats found"
		end

		local statObj = leaderstats:FindFirstChild(cmd.stat :: string)
		if not statObj then
			return false, "Stat not found: " .. tostring(cmd.stat)
		end

		local value = (statObj :: any).Value
		local actual = convertValue(value)
		local pass = applyOperator(cmd.operator :: string, actual, cmd.expected)
		return pass, actual
	end

	return false, "Unknown check type: " .. tostring(checkType)
end

--------------------------------------------------------------------------------
-- Main entry point
--------------------------------------------------------------------------------

function Verifier.check(cmd: CheckCommand): CheckResult
	local startTime = os.clock()

	local pass, actual = executeCheck(cmd)
	local elapsed = os.clock() - startTime

	-- If check passed or no timeout, return immediately
	local timeout = cmd.timeout or 0
	if pass or timeout <= 0 then
		return {
			pass = pass,
			message = if pass
				then "Check passed: " .. tostring(cmd.type)
				else "Check failed: " .. tostring(cmd.type),
			actual = actual,
			expected = cmd.expected,
			elapsed = elapsed,
			error = if not pass and type(actual) == "string" and not pass then actual else nil,
		}
	end

	-- Poll until pass or timeout
	while elapsed < timeout do
		task.wait(0.1)
		pass, actual = executeCheck(cmd)
		elapsed = os.clock() - startTime
		if pass then
			break
		end
	end

	return {
		pass = pass,
		message = if pass
			then "Check passed: " .. tostring(cmd.type) .. " (after " .. string.format("%.2f", elapsed) .. "s)"
			else "Check failed: " .. tostring(cmd.type) .. " (timed out after " .. string.format("%.2f", elapsed) .. "s)",
		actual = actual,
		expected = cmd.expected,
		elapsed = elapsed,
		error = if not pass and type(actual) == "string" then actual else nil,
	}
end

return Verifier
