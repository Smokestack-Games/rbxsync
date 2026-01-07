local sp = script.Parent
local jumper = script.Parent.Parent.Parent.DetectJump
local percent = game.Players.LocalPlayer.PlayerScripts:WaitForChild("Percent")
local UserInputService = game:GetService("UserInputService")

-- Store the initial button position
local initialPosition = sp.Position

local mouseInsideButton = false
local isMousePressed = false

local function moveButton(mouseX)
	if isMousePressed then
		local bar = sp.Parent
		local barLeft = bar.AbsolutePosition.X
		local barRight = bar.AbsolutePosition.X + bar.AbsoluteSize.X
		local newPosition = math.clamp((mouseX - barLeft) / bar.AbsoluteSize.X, 0, 1)
		sp.Position = UDim2.new(newPosition, 0, initialPosition.Y.Scale, initialPosition.Y.Offset)
		percent.Value = newPosition
		jumper.Position = UDim2.new(.5, 0, .5, 0)
		sp.Parent.Parent.BarVisual.Size = UDim2.new(0, math.floor((percent.Value) * 255), 0, 10)
	end
end

local Players = game:GetService("Players")
local player = Players.LocalPlayer
local mouse = player:GetMouse()

local function MouseBetweenPoints(pointA,pointB)
	local mouseVector = Vector2.new(mouse.X,mouse.Y)
	local pointAVector = Vector2.new(pointA.X.Offset,pointA.Y.Offset)
	local pointBVector = Vector2.new(pointB.X.Offset,pointB.Y.Offset)
	return ((mouseVector.X > pointAVector.X and mouseVector.Y > pointAVector.Y) and (mouseVector.X < pointBVector.X and mouseVector.Y < pointBVector.Y))
end

local function MouseInFrame(frame)
	local pointAVector = frame.AbsolutePosition
	local pointBVector = frame.AbsolutePosition + frame.AbsoluteSize
	return MouseBetweenPoints(UDim2.fromOffset(pointAVector.X,pointAVector.Y),UDim2.fromOffset(pointBVector.X,pointBVector.Y))
end

local RunService = game:GetService("RunService")

local mouseListenerConnection
mouseListenerConnection = RunService.RenderStepped:Connect(function()
	if MouseInFrame(jumper) then
		mouseInsideButton = true
		local mouseListenerConnection2
		mouseListenerConnection2 = RunService.RenderStepped:Connect(function()
			if MouseInFrame(jumper) == false then
				-- mouse left code
				mouseInsideButton = false
				mouseListenerConnection2:Disconnect()
			end
		end)
		
	end
end)

--jumper.MouseLeave:Connect(function()
--	mouseInsideButton = false
--end)

jumper.MouseButton1Down:Connect(function()
	isMousePressed = true
	--script.Click:Play()
	local mouseLocation = UserInputService:GetMouseLocation()
	moveButton(mouseLocation.X)
end)

UserInputService.InputChanged:Connect(function(input)
	if input.UserInputType == Enum.UserInputType.MouseMovement or input.UserInputType == Enum.UserInputType.Touch and mouseInsideButton == true then
		moveButton(input.Position.X)
	end
end)

UserInputService.InputEnded:Connect(function(input)
	if input.UserInputType == Enum.UserInputType.MouseButton1 then
		isMousePressed = false
		--script.ClickUp:Play()
	end
end)

-- Function to change the bar color based on percentage
function changeBarColor()
	local bar = sp.Parent.Parent.BarVisual
	local percentValue = percent.Value
	local barColor = Color3.new(0, 1, 0)  -- Initialize as green

	if percentValue >= .5 and percentValue < .75 then
		local blueValue = (percentValue - .50) / .25
		barColor = Color3.new(1 - blueValue, 0, blueValue)  -- Red to blue gradient
	elseif percentValue >= .75 then
		local purpleValue = (percentValue - .75) / .25
		barColor = Color3.new(purpleValue, 0, 1) -- Blue to purple
	else
		local greenValue = percentValue / .50
		barColor = Color3.new(greenValue, 1 - greenValue, 0)  -- Green to red gradient
	end

	bar.BackgroundColor3 = barColor
	local playerData = game.Players.LocalPlayer:FindFirstChild("leaderstats")
	if playerData == nil then
		return
	end
	--bar.Parent.Parent.Display.TroopCount.Text = tostring(math.floor((percentValue) * playerData.Troops.Value))
end

-- Call the function initially and whenever the percentage value changes
percent.Changed:Connect(changeBarColor)
changeBarColor()
