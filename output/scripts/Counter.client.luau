local sp = script.Parent
local jumper = script.Parent.Parent.Parent.DetectJump
local percent = game.Players.LocalPlayer.PlayerScripts:WaitForChild("Percent")
local UserInputService = game:GetService("UserInputService")

-- Store the initial button position
local initialPosition = sp.Position

local mouseInsideButton = false
local isMousePressed = false

local IsType = (function (T)
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
end)


-- get Troops
local function getTroopValue(player)
	local isBot = typeof(player) ~= "Instance"

	if isBot then

		return game.ReplicatedStorage.BotData[tostring(player)].Troops.Value
	else
		return player.leaderstats.Troops.Value
	end
end

-- get land
local function getLandValue(player)
	local isBot = typeof(player) ~= "Instance"

	if isBot then

		return game.ReplicatedStorage.BotData[tostring(player)].Land.Value
	else
		return player.leaderstats.Land.Value
	end
end

local TotalTroops = sp.Parent.Parent.Parent.Parent:WaitForChild("TotalTroops")

local Display = TotalTroops:WaitForChild("Display")

wait(.5)
while task.wait() do
	
	local percentValue = percent.Value
	local playerData = game.Players.LocalPlayer:FindFirstChild("leaderstats")
	if playerData == nil then
		--warn("no leaderstats found")
		
	else
	
	--	bar.Parent.Parent.Display.TroopCount.Text =  (IsType(math.floor((percentValue) * playerData.Troops.Value))) .. " (" .. (math.floor((percentValue * 100)) .. "%)")
	
		Display.TroopCount.Text = IsType(playerData.Troops.Value)
		Display.TickDisplay.TickBar.Size = UDim2.new(
			(((game.ReplicatedStorage.Data.CurrentTick.Value + 1 )% 10) + 1) /10
			 ,0,1,0)
			
		-- do math to find the point where density x = interest starts to decrease to 0
		-- bar tweens between max interest and this point, so players are encouraged to take action when density is optimal
		
		
		-- fetch player interest
		local troops = getTroopValue(game.Players.LocalPlayer)
		local land = getLandValue(game.Players.LocalPlayer)

		local density = troops/land
		
		local barSize 
		if density > 100 then
			barSize = 1
		else
			barSize = density/100
		end
		--((interest - 2.6) * -1)
		
		

		-- 2.6 is 0 size, 1 is 1 size
		TotalTroops.Display.DensityDisplay.TickBar.Size = UDim2.new(
			
			1,0,barSize,0)
	end
end
