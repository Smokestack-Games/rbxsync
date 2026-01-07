local MainUI = script.Parent

local EditButton = MainUI.SideButtons.Edit
local StatsButton = MainUI.SideButtons.Stats
local SkinsButton = MainUI.SideButtons.Skins

local PlayerUI = MainUI.Parent

local mapWindow = PlayerUI:WaitForChild("MapSelect")
local statWindow = PlayerUI:WaitForChild("Stats")
local skinWindow = PlayerUI:WaitForChild("Skins")
local editWindow = PlayerUI:WaitForChild("Edit")

EditButton.MouseButton1Down:Connect(function()
	
	-- check visible windows including self
	if mapWindow.Enabled == true then
		mapWindow.Enabled = false
		editWindow.Enabled = true
	elseif statWindow.Enabled == true then
		statWindow.Enabled = false
		editWindow.Enabled = true
	elseif skinWindow.Enabled == true then
		skinWindow.Enabled = false
		editWindow.Enabled = true
	elseif editWindow.Enabled == true then
		-- toggle the window
		editWindow.Enabled = false
		mapWindow.Enabled = true
	else
		print("No visible windows, continuing to display")
		editWindow.Enabled = true
	end
			
end)

SkinsButton.MouseButton1Down:Connect(function()

	-- check visible windows including self
	if mapWindow.Enabled == true then
		mapWindow.Enabled = false
		skinWindow.Enabled = true
	elseif statWindow.Enabled == true then
		statWindow.Enabled = false
		skinWindow.Enabled = true
	elseif editWindow.Enabled == true then
		editWindow.Enabled = false
		skinWindow.Enabled = true
	elseif skinWindow.Enabled == true then
		-- toggle the window
		skinWindow.Enabled = false
		mapWindow.Enabled = true
	else
		print("No visible windows, continuing to display")
		skinWindow.Enabled = true
	end

end)

StatsButton.MouseButton1Down:Connect(function()

	-- check visible windows including self
	if mapWindow.Enabled == true then
		mapWindow.Enabled = false
		statWindow.Enabled = true
	elseif editWindow.Enabled == true then
		editWindow.Enabled = false
		statWindow.Enabled = true
	elseif skinWindow.Enabled == true then
		skinWindow.Enabled = false
		statWindow.Enabled = true
	elseif statWindow.Enabled == true then
		-- toggle the window
		statWindow.Enabled = false
		mapWindow.Enabled = true
	else
		print("No visible windows, continuing to display")
		statWindow.Enabled = true
	end

end)
