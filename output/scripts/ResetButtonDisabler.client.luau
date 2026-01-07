-- Roblox Services
local starterGUI = game:GetService("StarterGui")

-- Disables the Reset Button
----[ Creates a Loop to make sure that the ResetButtonCallBack works.
repeat 
	local success = pcall(function() 
		starterGUI:SetCore("ResetButtonCallback", false) 
	end)
	task.wait(1)
until success