local Remotes = game.ReplicatedStorage.Remotes
Remotes.RespawnEvent.OnServerEvent:Connect(function(player)
	
	local function makePlayerHalfTransparent(player)
		local character = player.Character
		if character then
			for _, part in ipairs(character:GetDescendants()) do
				if part:IsA("BasePart") and part.Name ~= "HumanoidRootPart" then
					part.Transparency = 0.5  -- Set transparency to 0.5 for half transparency
				end
			end
		end
	end
	
	player:LoadCharacter()
	makePlayerHalfTransparent(player)
	
end)