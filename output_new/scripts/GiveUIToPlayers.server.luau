local Players = game:GetService("Players")
local StarterGui = game:GetService("StarterGui")

local function playerAdded(player)
	local playerGui = player:WaitForChild("PlayerGui")
	for _, gui in ipairs(StarterGui:GetChildren()) do
		gui:Clone().Parent = playerGui
	end
end

Players.PlayerAdded:Connect(playerAdded)
for _, player in ipairs(Players:GetPlayers()) do
	playerAdded(player)
end