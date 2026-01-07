local PlayerUI = script.Parent.Parent
local Player = game.Players.LocalPlayer
local RemoteFunction = game.ReplicatedStorage:WaitForChild("PlayerDataFunction")

local MainUI = script.Parent
local Window = MainUI.Window
local CloseButton = Window.CloseWindow.ImageButton
local NameWindow = Window:WaitForChild("Name")
local TextBox = NameWindow.Frame.TextBox
local ColorGrid = Window:WaitForChild("Color")

local defaultColor = Color3.new(255,255,255) -- This should be Color3.new(1,1,1) for normalized RGB values.

local function loadData()
	local data = RemoteFunction:InvokeServer("get")
	if data then
		TextBox.Text = data.Name or ""
		
		if type(data.Color) == "table" and #data.Color == 3 and
			type(data.Color[1]) == "number" and type(data.Color[2]) == "number" and type(data.Color[3]) == "number" then
			ColorGrid.Color.Value = BrickColor.new(Color3.new(unpack(data.Color)))
		else
			ColorGrid.Color.Value = BrickColor.new(Color3.new(unpack(data.Color)))
		end
		
		-- when implementing, do this
		local flag = data.Flag
		local skin = data.Skin
		
		Player:SetAttribute("DataLoaded", true)
	end
end

local function saveData()
	local playerName = TextBox.Text
	local data = {
		Name = TextBox.Text,
		Color = {ColorGrid.Color.Value.Color.R, ColorGrid.Color.Value.Color.G, ColorGrid.Color.Value.Color.B},
		
		Flag = nil, -- not implemented
		Skin = nil -- not implemented
	}
	local success = RemoteFunction:InvokeServer("set", data)
	if not success then
		warn("Failed to save data to the server.")
	end
end

ColorGrid.Color.Changed:Connect(function()
	NameWindow.ImageLabel.BackgroundColor = ColorGrid.Color.Value
	NameWindow.ImageLabel.UIStroke.Color = ColorGrid.Color.Value.Color
	saveData()
end)

--TextBox.Changed:Connect(saveData)

MainUI.Changed:Connect(saveData)

CloseButton.MouseButton1Down:Connect(function()
	MainUI.Enabled = false
	PlayerUI.MapSelect.Enabled = true
	saveData()
end)

loadData()
