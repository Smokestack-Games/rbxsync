-- Services
local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local UserInputService = game:GetService("UserInputService")
local GuiService = game:GetService("GuiService")

-- References
local ButtonEvent = ReplicatedStorage.Remotes.ButtonEvent
local TerritorySelectEvent = ReplicatedStorage.Remotes.TerritorySelectEvent
local GetPixelTypeFunction = ReplicatedStorage.Remotes.GetPixelTypeFunction

local player = Players.LocalPlayer
local mouse = player:GetMouse()
local ToggleCursor = script.Parent:WaitForChild("ToggleCursor")

-- GUI References
local playerGui = player:WaitForChild("PlayerGui")
local actionGui = player:WaitForChild("PlayerGui"):WaitForChild("Actions")
local buttonList = actionGui.ButtonList
local template = actionGui.Template
local buttons = template:GetChildren()
local guiInset = GuiService:GetGuiInset()

local buttonDictionary = {}
for _, button in buttons do
	buttonDictionary[button.Name] = button
	buttons = buttonDictionary
end

-- Constants
local BUTTON_SIZE = 64
local PADDING = 12

local storedPosition = mouse.Hit.p


local function timestamp()
	return DateTime.now().UnixTimestampMillis
end

local lastPointerDown = timestamp()
local clickTimeThreshold = 250

-- Functions

-- load elements
task.wait(1)
-- Reparents buttons back to template and hides the button list when the mouse leaves the area
local function ParentButtonsOnMouseOff()
	ToggleCursor.Value = true
	buttonList.Visible = false
	for _, button in buttons do
		button.Parent = template
	end
end

-- Adjusts the frame size based on the number of buttons
local function ResizeFrameFromButtonCount(buttonCount)
	local frameHeight = (BUTTON_SIZE + PADDING) * buttonCount
	buttonList.Size = UDim2.new(0, BUTTON_SIZE, 0, frameHeight)
end

-- Sets the position of the frame and makes it visible
local function PositionAndMakeFrameVisible(buttonCount)
	ResizeFrameFromButtonCount(buttonCount)
	local offset = 0
	if buttonCount == 2 then
		offset = 32+6
	end
	ToggleCursor.Value = false
	buttonList.Position = UDim2.new(0, mouse.X, 0, mouse.Y - offset)
	buttonList.Visible = true
end



-- Displays appropriate buttons based on game status and pixel type
local function ParentButtonTypesFromData(gameStatus, pixelType)
	ParentButtonsOnMouseOff()

	local buttonCount = 0
	-- print("pixelType: " .. pixelType)

	local buttonsToParent = TerritorySelectEvent:InvokeServer(player.UserId, pixelType, mouse.Hit.p)

	for _, buttonName in buttonsToParent do
		local button = buttons[buttonName]
		if buttonName == nil then continue end
		button.Parent = buttonList
		buttonCount += 1
	end

	if buttonCount > 0 then
		PositionAndMakeFrameVisible(buttonCount)
	end
end

-- Event Handlers
task.wait(1)
-- Reparent buttons when mouse leaves the button list
buttonList.MouseLeave:Connect(ParentButtonsOnMouseOff)

-- Connect button clicks to appropriate remote events
for _, button in buttons do
	button.MouseButton1Down:Connect(function()
		ButtonEvent:FireServer(button.Name, storedPosition)

	end)
end

local function getIndexFromPosition(x, y)
	return x * 250 + y + 1
end

local function getPointerTarget(inuputPos: Vector2)
	local raycastParams = RaycastParams.new()
	local ray = workspace.CurrentCamera:ScreenPointToRay(inuputPos.X, inuputPos.Y)
	local raycastResult = workspace:Raycast(ray.Origin, ray.Direction * 1000, raycastParams)
	return (raycastResult and raycastResult.Instance) or nil
end

local function isPointer(inputObject: InputObject): boolean
	return inputObject.UserInputType == Enum.UserInputType.MouseButton1 or inputObject.UserInputType == Enum.UserInputType.Touch
end

local function offsetLastPointerDown()
	lastPointerDown -= 251
end

local function areGuiButtonsBlockingInput()
	local mousePosition = UserInputService:GetMouseLocation()
	local guiObjects = playerGui:GetGuiObjectsAtPosition(mousePosition.X, mousePosition.Y - guiInset.Y)

	for _, guiObject in guiObjects do
		if guiObject:IsA("GuiButton") then
			offsetLastPointerDown()
			return true
		end
	end

	return false
end

local function canProceedWithClick()
	if areGuiButtonsBlockingInput() then return false end
	local lastTime = timestamp()
	local storeLastPointerDown = lastPointerDown

	if lastTime - storeLastPointerDown > clickTimeThreshold then
		return false
	end

	return true
end

local function onPointerPressed(inputPos: Vector2)
	if not canProceedWithClick() then return end

	local pointerTarget = getPointerTarget(inputPos)

	if pointerTarget and not pointerTarget:IsA("GuiObject") then
		offsetLastPointerDown()
		local pixelType = GetPixelTypeFunction:InvokeServer(mouse.Hit.p)
		local gameStatus = ReplicatedStorage.Data.GameStatus.Value
		print(gameStatus)
		ParentButtonTypesFromData(gameStatus, pixelType)
		storedPosition = mouse.Hit.p
		print("pixelIndex: " .. getIndexFromPosition(math.floor(mouse.Hit.p.X + .5), math.floor(mouse.Hit.p.Z+ .5)))
	end
end

local function hookEvents()
	UserInputService.InputBegan:Connect(function(inputObject: InputObject)
		if inputObject.UserInputType == Enum.UserInputType.MouseMovement then
			offsetLastPointerDown()
			return
		end

		-- ParentButtonsOnMouseOff()
		if isPointer(inputObject) == false then return end
		lastPointerDown = timestamp()
	end)

	UserInputService.InputEnded:Connect(function(inputObject: InputObject)
		ParentButtonsOnMouseOff()
		if isPointer(inputObject) == false then return end
		if canProceedWithClick() == false then return end
		local inputPos = inputObject.Position
		onPointerPressed(inputPos)
	end)
end

hookEvents()

-- Main mouse click logic
-- mouse.Button1Down:Connect(function()

-- 	if mouse.Target and not mouse.Target:IsA("GuiObject") then

-- 		local pixelType = GetPixelTypeFunction:InvokeServer(mouse.Hit.p)
-- 		local gameStatus = ReplicatedStorage.Data.GameStatus.Value
-- 		-- print(gameStatus)
-- 		ParentButtonTypesFromData(gameStatus, pixelType)
-- 		storedPosition = mouse.Hit.p
-- 		-- print("pixelIndex: " .. getIndexFromPosition(math.floor(mouse.Hit.p.X + .5), math.floor(mouse.Hit.p.Z+ .5)))
-- 		--print()
-- 	end
-- end)

ReplicatedStorage.Remotes.GetPercentAllocatedFunction.OnClientInvoke = function()
	return player.PlayerScripts.Percent.Value
end