-- Services and references
local Players = game:GetService("Players")
local mouse = Players.LocalPlayer:GetMouse()
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local GameStatus = ReplicatedStorage.Data:WaitForChild("GameStatus")

-- Function to round a Vector3 to the nearest integer
local function roundVector3(vec)
	return Vector3.new(math.floor(vec.x + 0.5), 1.5, math.floor(vec.z + 0.5))
end

-- Function to create a part with the given properties
local function createPart(size, color)
	local part = Instance.new("Part")
	part.Size = size
	part.Anchored = true
	part.BrickColor = BrickColor.new(color)
	part.CanCollide = false
	part.CanTouch = false
	part.Transparency = 1
	return part
end

-- Create highlight and cursor parts
local highlightPart1 = createPart(Vector3.new(3, .2, 1), "Bright blue")
local highlightPart2 = createPart(Vector3.new(1, .2, 3), "Bright blue")
local cursorPart = createPart(Vector3.new(1, .2, 1), "Bright blue")

-- Create a folder for ignored parts and set mouse's TargetFilter
local ignoredPartsFolder = Instance.new("Folder")
ignoredPartsFolder.Name = "IgnoredParts"
ignoredPartsFolder.Parent = workspace
workspace.IgnoredWalls.Parent = ignoredPartsFolder
highlightPart1.Parent = ignoredPartsFolder
highlightPart2.Parent = ignoredPartsFolder
cursorPart.Parent = ignoredPartsFolder
mouse.TargetFilter = ignoredPartsFolder

-- Function to create a selection box for a part
local function createSelectionBox(part, color)
	local selection = Instance.new("SelectionBox", part)
	selection.Adornee = part
	selection.Color3 = color
	return selection
end

-- Create SelectionBoxes
local spawnHi1 = createSelectionBox(highlightPart1, Color3.new(0, 1, 0.974182))
local spawnHi2 = createSelectionBox(highlightPart2, Color3.new(0, 1, 0.974182))
local cursorSelection = createSelectionBox(cursorPart, Color3.new(0, 1, 0.974182))

-- Function to update highlight and cursor parts' position
local function updateHighlightPosition()
	
	local position = roundVector3(mouse.Hit.Position)
	local outOfBounds = position.X < 0 or position.X > 249 or position.Z < 0 or position.Z > 249

	if outOfBounds then
	--	print(position)
		return
	end
	--print(position.X, position.Z)
	if GameStatus.Value == 1 then
		if outOfBounds then
			spawnHi1.Visible = false
			spawnHi2.Visible = false
			return
		else
			spawnHi1.Visible = true
			spawnHi2.Visible = true
		end
		highlightPart1.Position = position
		highlightPart2.Position = position
		cursorPart.Position = position
	elseif GameStatus.Value == 2 then
		if outOfBounds then
			cursorSelection.Visible = false
			return
		else
			cursorSelection.Visible = true
		end
		cursorPart.Position = position
		highlightPart1:Destroy()
		highlightPart2:Destroy()
		cursorSelection.Color3 = Color3.new(1, 0.149126, 0)
	elseif GameStatus.Value == 3 then
		cursorPart:Destroy()
	end
end

-- Update highlight and cursor positions when the cursor is toggled
local ToggleCursor = script.Parent:WaitForChild("ToggleCursor")
while task.wait() do
	if ToggleCursor.Value then
		updateHighlightPosition()
	end
end
