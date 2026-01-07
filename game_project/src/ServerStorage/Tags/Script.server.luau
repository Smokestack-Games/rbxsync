-- LocalScript within the template tag
local mapData = require(game.ReplicatedStorage:WaitForChild("MapData"))
local pixelData = mapData.pixelData
local tag = script.Parent
local playerNameLabel = tag.PlayerName
local troopCountLabel = tag.TroopCount
local playerId = tag.playerId.Value
local Nation = require(game.ReplicatedStorage.Classes.Nation)
local partData = require(game.ReplicatedStorage.PartData)

-- Variables to keep track of the connection to avoid multiple connections
local colorChangeConnection

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

local function updateTroopCount(newTroopCount)
	troopCountLabel.Text = IsType(newTroopCount)
end

-- Disconnect the old color change connection if it exists
local function disconnectColorChange()
	if colorChangeConnection then
		colorChangeConnection:Disconnect()
		colorChangeConnection = nil
	end
end



-- Function to find the median index part, assuming `partData` is available and has the necessary functions
local function findMedianPart()
	local playerNation = Nation.byPlayerId[playerId]
	local borderIndices = playerNation.borderPixels -- Your border indices
	
	if not borderIndices[1] then
		tag.Enabled = false
		--script.Enabled = false
		return
	end
	-- Constants
	local width = 250 -- Assuming the width of the grid is 250 pixels

	-- Helper function to convert index to x, y coordinates
	local function indexToPosition(index)
		local x = (index - 1) % width
		local y = math.floor((index - 1) / width)
		return x, y
	end

	-- Helper function to convert x, y coordinates to index
	local function positionToIndex(x, y)
		return x + y * width + 1
	end

	local function isPointInsideBorders(borderIndices, pointX, pointY)
		-- This function should check if the point is within the borders
		-- For a simple approximation, check if the point is not a border itself
		local index = positionToIndex(pointX, pointY)
		return not borderIndices[index]
	end

	-- Variables to keep track of the sum of x and y coordinates
	local sumX = 0
	local sumY = 0

	-- Iterate through border indices to sum coordinates
	for _, index in ipairs(borderIndices) do
		if pixelData[index] == playerId then
			local x, y = indexToPosition(index)
			sumX = sumX + x
			sumY = sumY + y
		end
	end

	-- Calculate the average coordinates
	local averageX = sumX / #borderIndices
	local averageY = sumY / #borderIndices

	-- Convert the average coordinates back to an index
	local averageIndex = positionToIndex(math.floor(averageX), math.floor(averageY))


	local adorneePart = partData.GetPart(averageIndex) -- Get the median part based on the center index
	if adorneePart then
		if pixelData[averageIndex] == playerId  then
			disconnectColorChange() -- Disconnect previous color change connection
			tag.Adornee = adorneePart -- Set the new adornee
			tag.Enabled = true
			-- Connect to the color change signal of the new adorneePart
			colorChangeConnection = adorneePart:GetPropertyChangedSignal("Color"):Connect(findMedianPart)
		else
			--local part
			for _,index in ipairs(borderIndices) do
				if pixelData[index] == playerId then
					adorneePart = partData.GetPart(index)
				end
			end
			if adorneePart then
				disconnectColorChange() -- Disconnect previous color change connection
				tag.Adornee = adorneePart -- Set the new adornee
				tag.Enabled = true
				-- Connect to the color change signal of the new adorneePart
				colorChangeConnection = adorneePart:GetPropertyChangedSignal("Color"):Connect(findMedianPart)
			else
				tag.Enabled = false
			end
			
		end	
	else
		-- do nothing
		tag.Enabled = false
	end
end

-- Call findMedianPart initially to set everything up
findMedianPart()

-- Listen for changes in troop count and update the label
local function setupTroopValueListener()
	local player
	local troopValue
	if tag.isBot.Value then
		-- If bot, listen to the respective bot's troop value
		player = game.ReplicatedStorage.BotData:FindFirstChild(playerId)
		troopValue = player:FindFirstChild("Troops")
	else
		-- If not a bot, listen to the player's troop value
		player = game.Players:GetPlayerByUserId(playerId)
		troopValue = player.leaderstats:FindFirstChild("Troops")
	end

	if player then
		if troopValue then
			troopValue:GetPropertyChangedSignal("Value"):Connect(function()
				updateTroopCount(troopValue.Value)
			end)
		end
	end
end

-- Set up the listener for troop value changes
setupTroopValueListener()



-- Clean up when the tag is destroyed
tag.AncestryChanged:Connect(function(_, parent)
	if not parent then
		disconnectColorChange() -- Clean up the color change connection
	end
end)

local storedColor

while task.wait() do
	if  pixelData[tonumber(tag.Adornee.Name)] ~= playerId then
		findMedianPart()
	end
end