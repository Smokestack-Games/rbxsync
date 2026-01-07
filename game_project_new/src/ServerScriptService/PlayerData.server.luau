local DataStoreService = game:GetService("DataStoreService")
local PlayerStore = DataStoreService:GetDataStore("PlayerPreferences")
local Players = game:GetService("Players")

local RemoteFunction
if game.ReplicatedStorage:FindFirstChild("PlayerDataFunction") then
	RemoteFunction = game.ReplicatedStorage.PlayerDataFunction
else
	RemoteFunction = Instance.new("RemoteFunction")
	RemoteFunction.Name = "PlayerDataFunction"
	RemoteFunction.Parent = game.ReplicatedStorage
end

RemoteFunction.OnServerInvoke = function(player, action, data)
	if action == "get" then
		local success, result = pcall(function()
			local data = PlayerStore:GetAsync(player.UserId)
			
			if not data.Name or data.Name == nil or data.Name == "" then
				data.Name = player.Name
			end
			return data
		end)
		if success then
			return result
		else
			warn("Failed to retrieve player data.")
			return nil
		end
	elseif action == "set" and data then
		local success, errorMessage = pcall(function()
			PlayerStore:SetAsync(player.UserId, data)
		end)
		return success
	end
end

Players.PlayerAdded:Connect(function(player)
	player:SetAttribute("DataLoaded", false)
end)
