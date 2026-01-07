local PlayerUI = script.Parent.Parent

local MapWindow = PlayerUI:WaitForChild("MapSelect")
local MainUI = script.Parent
local Window = MainUI.Window

local CloseButton = Window.CloseWindow.ImageButton

-- close window
CloseButton.MouseButton1Down:Connect(function()

	MainUI.Enabled = false
	MapWindow.Enabled = true

end)
