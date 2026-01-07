--------------------------------------- SERVICES --------------------------------------
local UserInputService = game:GetService("UserInputService")
local GuiService = game:GetService("GuiService")
---------------------------------------------------------------------------------------

--------------------------------------- VARIABLES -------------------------------------
local localPlayer = game.Players.LocalPlayer
local PlayerGui = localPlayer.PlayerGui
local PlayerScripts = localPlayer.PlayerScripts
local sliderGui = PlayerGui.Slider
---------------------------------------------------------------------------------------

--------------------------------------- CONSTANTS -------------------------------------
local clamp = math.clamp
local guiInset = GuiService:GetGuiInset()
---------------------------------------------------------------------------------------

local BordersSlider = {}
BordersSlider.__index = BordersSlider

function BordersSlider.create(bar: GuiObject, captureButton: GuiButton, barButton: GuiButton, barVisual: GuiObject, troopCount: TextLabel)
	local self = setmetatable({
		Bar = bar;
		BarButton = barButton;
		BarVisual = barVisual;
		Active = false;
		CaptureButton = captureButton;
		TroopCount = troopCount;
		TroopsValue = localPlayer:WaitForChild("leaderstats"):WaitForChild("Troops");
		Percent = .5;
		PercentValue = PlayerScripts.Percent;
		ColorValues = {
			{
				Range = Vector2.new(0, .5);
				CalculateColor = function(percent: number)
					local greenValue = percent / .5
					return Color3.new(greenValue, 1 - greenValue, 0)
				end
			};
			{
				Range = Vector2.new(.5, .75);
				CalculateColor = function(percent: number)
					local blueValue = (percent - .5) / .25
					return Color3.new(1 - blueValue, 0, blueValue)
				end
			};
			{
				Range = Vector2.new(.75, 1);
				CalculateColor = function(percent: number)
					local purpleValue = (percent - .75) / .25
					return Color3.new(purpleValue, 0, 1)
				end
			};
		}
	}, BordersSlider)

	-- self:VisualizeInputCaptureBounds()
	self:HookEvents()
	self:Update(self.Percent)
	return self
end

function BordersSlider:Commify(T: string): string
	-- converts 1234567890 to "1234567890"
	T = tostring(T)

	return T
		-- converts "123456789" to "987654321"
		:reverse()

		-- replaces every 3 digits with the captured string (the 3 digits) + a comma
		-- so "321" becomes "321,", "654321" "654,321," and "987654321" "987,654,321,"
		:gsub("%d%d%d", "%1,")

		-- converts "987,654,321," to ",123,456,789"
		:reverse()

		-- replaces the comma at the start of the string with nothing
		-- ",123,456,789" to "123,456,789"
		:gsub("^,", "")
end

function BordersSlider:Update(percentOverride: number?)
	local mousePosition = UserInputService:GetMouseLocation()

	local barPosition = self.Bar.AbsolutePosition
	local barSize = self.Bar.AbsoluteSize

	local xScaleOffset = percentOverride or clamp((mousePosition.X - (barPosition.X)) / barSize.X, 0, 1)

	self.Percent = xScaleOffset
	self.PercentValue.Value = self.Percent

	self.BarButton.Position = UDim2.new(xScaleOffset, 0, 0.5, 0)
	self.BarVisual.Size = UDim2.new(xScaleOffset, 0, 1, 0)
	self:UpdateColor()
	self:UpdateText()
end

-- local function getTroopValue(player)
-- 	local isBot = typeof(player) ~= "Instance"

-- 	if isBot then

-- 		return game.ReplicatedStorage.BotData[tostring(player)].Troops.Value
-- 	else
-- 		return player.leaderstats.Troops.Value
-- 	end
-- end

function BordersSlider:GetTroops()
	return self.TroopsValue.Value
end

function BordersSlider:UpdateColor()
	local percent = self.Percent
	local selected = nil
	for _, colorValue in self.ColorValues do
		if percent >= colorValue.Range.X and percent <= colorValue.Range.Y then
			selected = colorValue
			break
		end
	end

	if selected then
		self.BarVisual.BackgroundColor3 = selected.CalculateColor(percent)
	end
end

function BordersSlider:UpdateText()
	local percentValue = math.floor(self.Percent * 100)
	local allocatedTroopsValue = math.floor(self.TroopsValue.Value * self.Percent)
	self.TroopCount.Text = allocatedTroopsValue .. " (" .. percentValue .. "%)"
end

function BordersSlider:VisualizeInputCaptureBounds()
	local mousePosition = UserInputService:GetMouseLocation()
	local capturePosition = self.CaptureButton.AbsolutePosition
	local captureSize = self.CaptureButton.AbsoluteSize

	-- Place a square on 4 corners of the capture button
	local frame = Instance.new("Frame")
	frame.AnchorPoint = Vector2.new(0.5, 0.5)
	frame.Size = UDim2.new(0, 10, 0, 10)
	frame.BackgroundColor3 = Color3.fromRGB(255, 0, 0)
	frame.BorderSizePixel = 0

	local function createCorner(absX, absY)
		local corner = frame:Clone()
		corner.Position = UDim2.new(0, absX, 0, absY)
		corner.Parent = sliderGui
	end

	createCorner(capturePosition.X, capturePosition.Y + guiInset.Y)
	createCorner(capturePosition.X + captureSize.X, capturePosition.Y + guiInset.Y)
	createCorner(capturePosition.X, capturePosition.Y + captureSize.Y + guiInset.Y)
	createCorner(capturePosition.X + captureSize.X, capturePosition.Y + captureSize.Y + guiInset.Y)
end

function BordersSlider:CanCaptureInput()
	local mousePosition = UserInputService:GetMouseLocation()
	local capturePosition = self.CaptureButton.AbsolutePosition
	local captureSize = self.CaptureButton.AbsoluteSize

	-- Check if the mouse is within the bounds of the capture button
	return mousePosition.X >= capturePosition.X and mousePosition.X <= capturePosition.X + captureSize.X and
		mousePosition.Y >= capturePosition.Y + guiInset.Y and mousePosition.Y <= capturePosition.Y + captureSize.Y + guiInset.Y
end



function BordersSlider:HookEvents()
	self:HookInput()
	self.TroopsValue.Changed:Connect(function()
		self:UpdateText()
	end)
end

function BordersSlider:HookInput()
	UserInputService.TouchStarted:Connect(function(inputObject)
		if inputObject.UserInputType == Enum.UserInputType.Touch and self:CanCaptureInput() then
			self.Active = true
			self:Update()
		end
	end)

	UserInputService.TouchMoved:Connect(function(inputObject: InputObject)
		if self.Active and inputObject.UserInputType == Enum.UserInputType.Touch then
			self:Update()
		end
	end)

	UserInputService.TouchEnded:Connect(function(inputObject: InputObject)
		if inputObject.UserInputType == Enum.UserInputType.Touch then
			self.Active = false
			if self:CanCaptureInput() then
				self:Update()
			end
		end
	end)

	UserInputService.InputBegan:Connect(function(inputObject: InputObject)
		if inputObject.UserInputType == Enum.UserInputType.MouseButton1 and self:CanCaptureInput() then
			self.Active = true
			self:Update()
		end
	end)

	UserInputService.InputChanged:Connect(function(inputObject: InputObject)
		if self.Active and inputObject.UserInputType == Enum.UserInputType.MouseMovement then
			self:Update()
		end
	end)

	UserInputService.InputEnded:Connect(function(inputObject: InputObject)
		if inputObject.UserInputType == Enum.UserInputType.MouseButton1 then
			self.Active = false
			if self:CanCaptureInput() then
				self:Update()
			end
		end
	end)
end

return BordersSlider