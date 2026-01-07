local Actions = {}

local ReplicatedStorage = game:GetService("ReplicatedStorage")
local mapData = require(ReplicatedStorage:WaitForChild("MapData"))
local partData = require(ReplicatedStorage:WaitForChild("PartData"))
local Nation = require(ReplicatedStorage.Classes:WaitForChild("Nation"))
local Calculate = require(script.Parent.Calculate)
local ReNotify = require(game.ServerScriptService.MainModule)  -- Assuming this is the correct module ID



local colorPart = partData.ColorPart
local texturePart = partData.TexturePart
local pixelData, width, height = mapData.pixelData, mapData.width, mapData.height


local function getBorderColor(color)
	return color:Lerp(Color3.new(0,0,0), 0.3)
end

local function getExpandColor(color)
	return color:Lerp(Color3.new(1, 1, 1), 0.3)
end

local function getNeighborPixels(pixelId)
	local neighbors = {}
	if pixelId % width ~= 0 then neighbors.right = pixelId + 1 end
	if pixelId % width ~= 1 then neighbors.left = pixelId - 1 end
	if pixelId <= (width * (height - 1)) then neighbors.bottom = pixelId + width end
	if pixelId > width then neighbors.top = pixelId - width end
	return neighbors
end

local function getAllNeighborPixels(pixelId)
	local neighbors = {}
	-- top
	if pixelId > width then
		table.insert(neighbors, pixelId - width)
	end

	-- bottom
	if pixelId <= (width * (height - 1)) then
		table.insert(neighbors, pixelId + width)
	end

	-- left
	if pixelId % width ~= 1 then
		table.insert(neighbors, pixelId - 1)
	end

	-- right
	if pixelId % width ~= 0 then
		table.insert(neighbors, pixelId + 1)
	end

	-- top left
	if pixelId > width and pixelId % width ~= 1 then
		table.insert(neighbors, pixelId - width - 1)
	end

	-- top right
	if pixelId > width and pixelId % width ~= 0 then
		table.insert(neighbors, pixelId - width + 1)
	end

	-- bottom left
	if pixelId <= (width * (height - 1)) and pixelId % width ~= 1 then
		table.insert(neighbors, pixelId + width - 1)
	end

	-- bottom right
	if pixelId <= (width * (height - 1)) and pixelId % width ~= 0 then
		table.insert(neighbors, pixelId + width + 1)
	end

	return neighbors
end

local function getLandValue(player)
	local isBot = typeof(player) ~= "Instance"

	if isBot then
		return game.ReplicatedStorage.BotData[tostring(player)].Land.Value
	else
		return player.leaderstats.Land.Value
	end

end

local function setLandValue(player, newValue)
	local isBot = typeof(player) ~= "Instance"
	if isBot then
		game.ReplicatedStorage.BotData[tostring(player)].Land.Value = newValue
	else
		player.leaderstats.Land.Value = newValue
	end
end

Actions.getLandValue = getLandValue
Actions.setLandVvalue = setLandValue

local function getTroopValue(player)
	local isBot = typeof(player) ~= "Instance"

	if isBot then
		return game.ReplicatedStorage.BotData[tostring(player)].Troops.Value
	else
		return player.leaderstats.Troops.Value
	end

end

local function updateLandValue(playerNation, landChange)
	local userId = playerNation.playerId
	local player = playerNation.isBot and game.ReplicatedStorage.BotData[tostring(userId)] or game.Players:GetPlayerByUserId(userId)
	local landStat = playerNation.isBot and player.Land or player.leaderstats.Land

	-- Adjust the land value safely, ensuring it never goes below zero
	landStat.Value = math.max(0, landStat.Value + landChange)
end

-- HANDLES A SINGLE EXPANSION UNIT
local function expandPixels(playerId, enemyId, isAttack)
	local playerNation = Nation.byPlayerId[playerId]
	local playerColor = playerNation.Color
	local borderColor = getBorderColor(playerColor)
	local expandColor = getExpandColor(playerColor)
	local newBorderPixels = {} -- Store new border pixels that are not part of the enemy territory
	local newEnemyBorderPixels = {}
	local enemyNation, enemyBorderColor

	local playerPixelsAdded = 0
	local enemyPixelsRemoved = 0

	if isAttack then
		enemyNation = Nation.byPlayerId[enemyId]
		enemyBorderColor = getExpandColor(enemyNation.Color)
		--	newBorderPixels = playerNation.borderPixels
	end

	for _, pixelId in ipairs(playerNation.borderPixels) do
		if pixelData[pixelId] == playerId then

			local ownNeighborCount = 0

			for direction, neighborPixel in pairs(getNeighborPixels(pixelId)) do
				local neighborValue = pixelData[neighborPixel]
				if neighborValue == enemyId then

					-- claim new border
					pixelData[neighborPixel] = playerId
					colorPart(neighborPixel, expandColor)
					playerPixelsAdded += 1
					table.insert(newBorderPixels, neighborPixel)

					-- handle attack borders logic
					if isAttack then	
						enemyPixelsRemoved += 1
						-- loop through neighbors to color new enemy border
						for direction, enemyNeighborPixel in pairs(getNeighborPixels(neighborPixel)) do
							local neighborValue = pixelData[enemyNeighborPixel]
							if neighborValue == enemyId then

								table.insert(newEnemyBorderPixels, enemyNeighborPixel)
								colorPart(enemyNeighborPixel, enemyBorderColor)
								--elseif neighborValue == playerId then
								--	colorPart(enemyNeighborPixel, playerColor)
							end
						end
					end


					ownNeighborCount += 1
				elseif neighborValue == playerId then
					ownNeighborCount += 1
				end
			end
			if ownNeighborCount < 4 then
				table.insert(newBorderPixels, pixelId)  -- If it remains a border pixel, keep it
			elseif pixelData[pixelId] == playerId then

				colorPart(pixelId, playerColor)
			end
		end
	end

	if enemyPixelsRemoved > 0 then

		updateLandValue(enemyNation, -enemyPixelsRemoved)


		for i,v in ipairs(newEnemyBorderPixels) do
			if Nation.byPlayerId[playerId].isBot == false then
				--warn("Inserting:", v)
			end
			table.insert(Nation.byPlayerId[enemyId].borderPixels, v)
		end
	end

	-- handling land increase/decrease

	updateLandValue(playerNation, playerPixelsAdded)

	playerNation.borderPixels = newBorderPixels
	return newBorderPixels, newEnemyBorderPixels
end




function getIndexFromPosition(x, y)
	return x * width + y + 1
end

function getPositionFromIndex(index)
	--index div by width gives x, the row, the remainder is y (+1)

	local y = (index) % width - 1 -- x is now the column index
	local x = math.floor((index) / width) -- y is now the row index
	return x, y
end

function bresenhamsLine(x0, y0, x1, y1)
	local dx = math.abs(x1 - x0)
	local sx = x0 < x1 and 1 or -1
	local dy = -math.abs(y1 - y0)
	local sy = y0 < y1 and 1 or -1
	local err = dx + dy
	local line = {}

	while true do
		table.insert(line, getIndexFromPosition(x0, y0))
		if x0 == x1 and y0 == y1 then break end
		local e2 = 2 * err
		if e2 >= dy then
			err = err + dy
			x0 = x0 + sx
		end
		if e2 <= dx then
			err = err + dx
			y0 = y0 + sy
		end
	end

	return line
end

function getLinePixels(pixelIndex1, pixelIndex2)
	local x0, y0 = getPositionFromIndex(pixelIndex1)
	--local part = Instance.new("Part")
	--local partClone = part:Clone()
	--partClone.Position =  Vector3.new(x0, 5, y0)


	--partClone.Parent = game.Workspace

	--local partClone2 = part:Clone()
	local x1, y1 = getPositionFromIndex(pixelIndex2)


	--partClone2.Position =  Vector3.new(x1, 5, y1)

	--partClone2.Parent = game.Workspace

	return bresenhamsLine(x0, y0, x1, y1)
end

function areAllPixelsWater(pixelIndex1, pixelIndex2)
	local x0, y0 = getPositionFromIndex(pixelIndex1)
	local x1, y1 = getPositionFromIndex(pixelIndex2)
	local linePixels = bresenhamsLine(x0, y0, x1, y1)

	--	print("made line, doing check now")
	for i, pixelIndex in ipairs(linePixels) do

		--colorPart(pixelIndex, Color3.new(1, 1, 0))

		--print(pixelIndex)
		if pixelData[pixelIndex] ~= 1 then
			print(tostring(pixelIndex) .. " is not water")
			return false
		end
	end

	return true
end




-- NEED TO ADD BOAT! CHECK NEIGHBORS FOR WATER AND CHECK CALCULATED PATH FROM SHORTEST POINT TO DESTINATION

-- Function to check if a pixel is water
local function isWaterPixel(pixelId)
	if pixelId == 1 then
		return true
	end
	return false
	-- Implement logic to determine if a pixel is water
	-- This might involve checking the pixel's type or color
end

-- Function to check if a pixel is adjacent to water
local function isAdjacentToWater(pixelId)
	for _, neighborPixel in getNeighborPixels(pixelId) do
		if pixelData[neighborPixel] == 1 then
			return neighborPixel
		end
	end
	return false
end

local function manhattanDistance(x1, y1, x2, y2)
	return math.abs(x1 - x2) + math.abs(y1 - y2)
end

-- Function to find the player's closest border pixel to a given pixel
local function findClosestPlayerBorder(playerNation, targetPixel)
	local closestBorderPixel = nil
	local minDistance = math.huge

	for _, borderPixel in ipairs(playerNation.borderPixels) do
		local x1, y1 = getPositionFromIndex(borderPixel)
		local x2, y2 = getPositionFromIndex(targetPixel)
		local distance = manhattanDistance(x1, y1, x2, y2)

		if distance < minDistance and pixelData[borderPixel] == playerNation.playerId then
			minDistance = distance
			closestBorderPixel = borderPixel
		end
	end

	return closestBorderPixel
end

local function isBoatable(playerId, pixelId)
	local playerNation = Nation.byPlayerId[playerId]
	local playerNationSharesBorderWithWater = playerNation:SharesBordersWith(1)
	if not playerNationSharesBorderWithWater then return false end

	local borderPixels = {}
	local exhausted = {}
	local startPixel = findClosestPlayerBorder(playerNation, pixelId)
	local done = false

	local originalStart = startPixel

	if not isAdjacentToWater(startPixel) then
		repeat
			local neighbors = getAllNeighborPixels(startPixel)
			local count = 0
			for _, neighborPixel in neighbors do
				table.insert(exhausted, neighborPixel)
				count += 1
				if table.find(playerNation.borderPixels, neighborPixel) and not table.find(exhausted, neighborPixel) then
					table.insert(borderPixels, neighborPixel)
					break
				end
			end
			if count == #neighbors then
				done = true
			end
		until done == true
	end

	-- sort by distance from original start
	local x1, y1 = getPositionFromIndex(originalStart)
	table.sort(borderPixels, function(a, b)
		local x2, y2 = getPositionFromIndex(a)
		local x3, y3 = getPositionFromIndex(b)
		return manhattanDistance(x1, y1, x2, y2) < manhattanDistance(x1, y1, x3, y3)
	end)

	for _, borderPixel in borderPixels do
		if isAdjacentToWater(borderPixel) then
			startPixel = borderPixel
			break
		end
	end

	if not isAdjacentToWater(startPixel) then
		return false
	end

	local endPixel = pixelId
	if startPixel == endPixel then return false end
	local line = getLinePixels(startPixel, endPixel)

	-- for _, pixel in line do
	--     partData.GetPart(pixel).Color = Color3.new(1, 0, 0)
	-- end

	local lastIndex = nil
	for i, pixel in line do
		if pixelData[pixel] > 1 and pixel ~= startPixel then
			endPixel = pixel
			lastIndex = i
			break
		end
	end

	-- partData.GetPart(startPixel).Color = Color3.new(1, 0, 0)
	-- partData.GetPart(endPixel).Color = Color3.new(1, 0, 0)

	if not isAdjacentToWater(endPixel) then
		local neighbors = getNeighborPixels(endPixel)
		for _, neighbor in neighbors do
			if pixelData[neighbor] ~= pixelData[endPixel] then continue end
			if isAdjacentToWater(neighbor) then
				endPixel = neighbor
				break
			end
		end
	end


	if not playerNationSharesBorderWithWater or not isAdjacentToWater(endPixel) then
		return false
	end

	local foundWater = false
	warn(lastIndex, line, startPixel, endPixel)
	for i = lastIndex, #line do
		if pixelData[line[i]] ~= 1 and foundWater == true then
			return false
		elseif pixelData[line[i]] == 1 then
			foundWater = true
		end
	end

	line = getLinePixels(startPixel, endPixel)

	return true, line, startPixel, endPixel
end

Actions.isBoatable = isBoatable



function Actions.CheckTerritoryButtons(playerId, pixelValue, pixelId)
	local gameStatus = ReplicatedStorage.Data.GameStatus.Value

	local actionsList = {}
	local playerNation = Nation.byPlayerId[playerId]
	local enemyNation = Nation.byPlayerId[pixelValue]

	warn("player nation:", playerNation, "enemy nation:", enemyNation)

	if pixelValue == 2 and gameStatus == 1 then
		table.insert(actionsList, "Spawn")
		return actionsList
	elseif pixelValue == 2 and gameStatus == 2 then
		local boatable, line = isBoatable(playerId, pixelId)
		warn("boatable:", boatable, "line:", line)
		if not boatable then
			-- if it's not a boatable situation:
			table.insert(actionsList, "Expand")
			return actionsList
		else
			-- if it's a boatable situation:
			table.insert(actionsList, "Boat")
			return actionsList
		end
	end

	if getLandValue(game.Players:GetPlayerByUserId(playerId)) == 0 then
		return actionsList
	end
	if pixelValue == playerId then
		return actionsList -- Self territory
	elseif enemyNation.Team == playerNation.Team then
		table.insert(actionsList, "Donate")
		return actionsList
	elseif playerNation:isAlly(pixelValue) then
		table.insert(actionsList, "Donate")
		if playerNation:SharesBordersWith(pixelValue) then
			table.insert(actionsList, "Attack")
			return actionsList
		end
	elseif playerNation:SharesBordersWith(pixelValue) then
		table.insert(actionsList, "Attack")
		if enemyNation.isBot == false and table.find(enemyNation.Allies, playerNation.playerId) == nil then
			table.insert(actionsList, "Ally")
		end
		return actionsList
	elseif enemyNation.isBot == false and table.find(enemyNation.Allies, playerNation.playerId) == nil then
		table.insert(actionsList, "Ally")
	end

	-- THIS IS FLAWED

	-- get closest player border pixel to selection
	-- draw path to selected pixel
	-- if water in way, prompt

	local startPixel = findClosestPlayerBorder(enemyNation, pixelId)

	local endPixel = pixelId


	-- CHECK FOR BOAT LOGIC

	if enemyNation:SharesBordersWith(1) or pixelValue == 2 then
		--print(tostring(enemyNation) .. " shares border with water")

		if playerNation:SharesBordersWith(1) then

			--print(tostring(playerNation) .. " shares border with water")
			--colorPart(startPixel, Color3.new(1, 0, 0))

			startPixel = isAdjacentToWater(startPixel)
			-- MUST CHECK IF THE BORDER PIXEL IS ADHACENT TO WATER FIRST
			if startPixel then
				--	print("check1")
				endPixel = findClosestPlayerBorder(playerNation, startPixel)


				--colorPart(startPixel, Color3.new(1, 1, 0))
				--colorPart(endPixel, Color3.new(1, 0, 0))

				endPixel = isAdjacentToWater(endPixel)



				if endPixel then
					--print("check2")
					--colorPart(endPixel, Color3.new(1, 1, 0))




					print(startPixel)
					print(endPixel)

					-- we flip these since endPixel is player, and startPixel is enemy
					if areAllPixelsWater(endPixel, startPixel) then
						print("ALL REQUIREMENTS SATISFIED TO SEND A BOAT")

						--print("start " .. tostring(startPixel))
						--print("end ".. tostring(endPixel))

						--colorPart(startPixel, Color3.new(1, 1, 0))
						--colorPart(endPixel, Color3.new(1, 1, 0))


						-- get neighbor pixel that is water?

						--print()


						-- i need some way to run all this logic again if the boat button is clicked
						-- we need to actually send the boat across and make it land
						-- needs to lose troops while sailing
						-- 
						table.insert(actionsList, "Boat")

					end


				end-- Add 'Boat' action if conditions are met
			end

		end

	end

	return actionsList -- No actions
end


local function getPercent(playerNation)
	if not playerNation.isBot then
		return math.max(0, math.min(1, game.ReplicatedStorage.Remotes.GetPercentAllocatedFunction:InvokeClient(game.Players:GetPlayerByUserId(playerNation.playerId))))
	else
		return math.random(30,95)/100
	end
end








-- Spawn helpers --

local templateTag = game.ServerStorage:WaitForChild("Tags") -- Assuming this is the template

local function createPlayerTag(playerNation)
	local tagClone = templateTag:Clone()
	tagClone.isBot.Value = playerNation.isBot
	--tagClone.Name.Value = player.Name
	if playerNation.isBot then
		tagClone.PlayerName.Text = "<u>" .. playerNation.Name .. "</u>" -- Assuming there's a text label for the player's name
	elseif playerNation.Name == "" or playerNation.Name == nil then
		-- default to player name
		local playerFetch = game.Players:GetPlayerByUserId(playerNation.playerId)
		tagClone.PlayerName.Text = playerFetch.Name 
	else
		tagClone.PlayerName.Text = playerNation.Name
	end

	tagClone.TroopCount.Text = "16" -- Default troop count
	tagClone.Name = playerNation.playerId
	tagClone.playerId.Value = playerNation.playerId
	-- Parent to workspace to make it visible

	-- Enabling the LocalScript inside the clone
	if tagClone.Script then
		tagClone.Script.Enabled = false
	end

	tagClone.Adornee = partData.GetPart(playerNation.borderPixels[1])
	tagClone.Parent = game.Workspace.Tags

	tagClone.Script.Enabled = true
	-- Return the clone to keep a reference if needed
	--return tagClone
end


local oldSpawnColors = {}
local oldSpawnTextures = {}

local function resetOldSpawn(playerNation)
	local oldSpawnId = playerNation.borderPixels[1]
	colorPart(oldSpawnId, oldSpawnColors[playerNation.playerId])
	texturePart(oldSpawnId, oldSpawnTextures[playerNation.playerId])
	pixelData[oldSpawnId] = 2
	table.remove(playerNation.borderPixels, 1)
end

local function setNewSpawn(playerNation, pixelId, borderColor)
	pixelData[pixelId] = playerNation.playerId
	table.insert(playerNation.borderPixels, pixelId)


	oldSpawnColors[playerNation.playerId] = partData.GetColorOfIndex(pixelId)
	oldSpawnTextures[playerNation.playerId] = partData.GetTextureOfIndex(pixelId)
	colorPart(pixelId, borderColor)


end

local function handleSpawn(playerNation, pixelId, isLand)
	local borderColor = getBorderColor(playerNation.Color)
	if isLand then

		-- handle leaderstats
		if playerNation.isBot then
			--==local player = game.ReplicatedStorage.BotData[tostring()]
			--if getLandValue(playerNation.playerId) > 0 then
			--	resetOldSpawn(playerNation)
			--	setNewSpawn(playerNation, pixelId, borderColor)
			--else
			--updateLandValue(playerNation, 1)

			setNewSpawn(playerNation, pixelId, borderColor)

			createPlayerTag(playerNation)
			--end
		else
			local player = game.Players:GetPlayerByUserId(playerNation.playerId)
			--if player:FindFirstChild("leaderstats") then
			if getLandValue(player) > 0 then
				resetOldSpawn(playerNation)
				setNewSpawn(playerNation, pixelId, borderColor)
			else
				updateLandValue(playerNation, 1)

				setNewSpawn(playerNation, pixelId, borderColor)
				createPlayerTag(playerNation)
			end
			--end
		end


	end
end


-- Calulate helpers

-- Helper to calculate the number of pixels available for expansion
local function getPixelsToConquer(playerNation)
	local pixelsToConquer = 0
	for _, pixelId in ipairs(playerNation.borderPixels) do
		for _, neighborPixel in pairs(getNeighborPixels(pixelId)) do
			if pixelData[neighborPixel] == 2 then
				pixelsToConquer += 1
			end
		end
	end
	return pixelsToConquer
end

-- Helper to calculate the border amount for attacks
local function getBorderAmount(playerNation, enemyNation)
	local borderAmount = 0
	for _, pixelId in ipairs(playerNation.borderPixels) do
		if pixelData[pixelId] == playerNation.playerId then
			-- fixing extra border count
			for _, neighborPixel in pairs(getNeighborPixels(pixelId)) do
				if pixelData[neighborPixel] == enemyNation.playerId then
					borderAmount += 1
				end
			end
		end
	end
	return borderAmount
end


-- update Troops
local function updateTroopValue(player, newValue)
	local isBot = typeof(player) ~= "Instance"

	if isBot then
		local oldTroops = game.ReplicatedStorage.BotData[tostring(player)].Troops
		oldTroops.Value = math.max(0, newValue)
	else
		local oldTroops = player.leaderstats.Troops
		oldTroops.Value = math.max(0, newValue)
	end	
end

-- Actions


function Actions.Spawn(playerId, pixelId)
	local playerNation = Nation.byPlayerId[playerId]
	handleSpawn(playerNation, pixelId, pixelData[pixelId] == 2)
end


local tickLength = ReplicatedStorage.Data.TickLength.Value

function Actions.Expand(playerId, callback, ...)
	if callback then
		callback( ... )
	end

	local playerNation = Nation.byPlayerId[playerId]
	local player = playerId
	local percent = getPercent(playerNation)

	if not playerNation.isBot then
		player = game.Players:GetPlayerByUserId(playerId)
	end

	local attackingTroops = math.floor(getTroopValue(player) * percent + .5) -- or however you calculate the initial attacking troops

	--get pixels to conquer somehow

	task.spawn(function()

		-- Loop to expand while there are enough troops
		local newAttackerTroops = getTroopValue(player)  - attackingTroops
		updateTroopValue(player, newAttackerTroops)

		local borderColor = getBorderColor(playerNation.Color)
		local expandColor = getExpandColor(playerNation.Color)
		--for i,v in playerNation.borderPixels do
		--	if pixelData[v] == playerId then
		--		colorPart(v, expandColor)
		--	end
		--end
		local newBorderPixels = {}
		local newEnemyBorderPixels
		--task.wait(tickLength)
		local expandCount = 0
		-- need to update troops first, calculate will return remaining troops
		while attackingTroops > 0 do

			local newTick = tickLength

			local pixelsToConquer = getPixelsToConquer(playerNation)

			-- double the density (2 for empty pixels)
			if attackingTroops/pixelsToConquer > 2 * 2 * pixelsToConquer then
				newTick = tickLength/2
			end

			if not playerNation.isBot then
				print("expanding with " .. attackingTroops .. "against " .. pixelsToConquer * 2)

			end

			if pixelsToConquer == 0 then
				-- still need to return troops!
				local expansionSuccessful, remainingTroops = Calculate.expandTerritory(player, pixelsToConquer, attackingTroops)
				break
			end
			local expansionSuccessful, remainingTroops = Calculate.expandTerritory(player, pixelsToConquer, attackingTroops)
			if not expansionSuccessful then 
				newBorderPixels = playerNation.borderPixels
				for i,v in newBorderPixels do
					--if pixelData[v] == playerId then
					colorPart(v, borderColor)
					--end
				end
				if expandCount == 0 then
					--notify of failed expansion, not enough troops
					ReNotify(player, "Not enough troops to expand!",
						{
							Type = "error";
							Timeout = 2;
						}
					)
				end
				break
			else
				expandCount = expandCount + 1
			end
			newBorderPixels, newEnemyBorderPixels = expandPixels(playerId, 2, false)
			attackingTroops = remainingTroops
			task.wait(newTick)

		end

		-- recolor borders at end
		--local borderColor = getBorderColor(playerNation.Color)
		--task.wait()
		for i,v in newBorderPixels do
			if pixelData[v] == playerId then
				colorPart(v, borderColor)
			end
		end

	end)


end

local function genericAttack()

end

function Actions.Attack(playerId, callback, ...)
	local enemyId = (callback or genericAttack)( ... )

	-- warn("enemy id:", enemyId)

	local playerNation = Nation.byPlayerId[playerId]
	local enemyNation = Nation.byPlayerId[enemyId]
	warn("enemy id:", enemyId, enemyNation)
	if enemyNation == nil then return false end
	local player = playerId
	local enemy = enemyId

	local percent = getPercent(playerNation)
	-- Attack logic 

	if not playerNation.isBot then
		player = game.Players:GetPlayerByUserId(playerId)

	end
	if not enemyNation.isBot then
		enemy = game.Players:GetPlayerByUserId(enemyId)
	end

	local attackingTroops = math.floor(getTroopValue(player) * percent + .5) -- or however you calculate the initial attacking troops

	--get pixels to conquer somehow
	--local pixelsToConquer = 1
	-- CALCULATE


	task.spawn(function()

		--local newAttackerTroops = getTroopValue(player)  - attackingTroops
		updateTroopValue(player, getTroopValue(player) - attackingTroops)

		-- need to update troops first, calculate will return remaining troops
		local borderColor = getBorderColor(playerNation.Color)
		local expandColor = getExpandColor(playerNation.Color)
		local enemyBorderColor = getBorderColor(enemyNation.Color)
		--for i,v in playerNation.borderPixels do
		--	if pixelData[v] == playerId then
		--		colorPart(v, expandColor)
		--	end
		--end

		local newBorderPixels = {}
		local newEnemyBorderPixels = {}
		--task.wait(tickLength)
		local newTick = tickLength
		local gate = true

		if not enemyNation.isBot then
			ReNotify(enemy, tostring(playerNation.Name .. " is attacking you with " .. attackingTroops .. " troops."),
				{
					Type = "error";
					Timeout = 2;
				}
			)
		end

		if not enemyNation.isBot and table.find(enemyNation.Allies, playerNation.playerId) then
			ReNotify(enemy, tostring(playerNation.Name .. " has broken your alliance!"),
				{
					Type = "critical";
					Timeout = 5;
				}
			)
			local findEnemyAlly = table.find(enemyNation.Allies, playerNation.playerId)
			if findEnemyAlly then
				table.remove(enemyNation.Allies, findEnemyAlly)
			end

			local findPlayerAlly = table.find(playerNation.Allies, enemyNation.playerId)
			if findPlayerAlly then
				table.remove(playerNation.Allies, findPlayerAlly)
			end
			--print(enemyNation:isAlly(playerNation.playerId))
			local player1 = game.Players:GetPlayerByUserId(enemyNation.playerId)
			local player2 = game.Players:GetPlayerByUserId(playerNation.playerId)
			local remote1 = player1:FindFirstChild("RemoteEvent")
			local remote2 = player2:FindFirstChild("RemoteEvent")
			remote1:FireClient(player1, playerNation.playerId, false)
			remote2:FireClient(player2, enemyNation.playerId, false)
		end

		while attackingTroops > 1 do

			task.wait(newTick)
			local touchingBorderCount =  getBorderAmount(playerNation, enemyNation)

			-- double the density 
			if attackingTroops/touchingBorderCount > 2 * Calculate.calculateBorderDensity(getTroopValue(enemy), getLandValue(enemy) ) and gate then
				gate = false
				newTick = tickLength/2
			end


			if not playerNation.isBot then
				--warn("attacking nation:", playerNation, enemyNation, touchingBorderCount, Calculate.calculateBorderDensity(getTroopValue(enemy), getLandValue(enemy) ))			
				--print("attacking with " .. attackingTroops .. "against " .. touchingBorderCount * Calculate.calculateBorderDensity(getTroopValue(enemy), getLandValue(enemy) ) )

			end


			--print(touchingBorderCount)
			-- UPDATE CLIENT HERE WITH THE REMAINING TROOPS, CLIENT WILL DISPLAY PERCENT BAR!


			local attackerLosses, defenderLosses, remainingTroops = Calculate.attack(player, enemy, attackingTroops, touchingBorderCount)
			-- fix this 
			if attackerLosses == 0 or defenderLosses == 0 or touchingBorderCount == 0 then 
				newBorderPixels = playerNation.borderPixels
				newEnemyBorderPixels = enemyNation.borderPixels
				for i,v in newBorderPixels do
					if pixelData[v] == playerId then
						colorPart(v, borderColor)
						--else

						--	colorPart(v, Nation.byPlayerId[pixelData[v]].Color)
					end
				end
				for i,v in newEnemyBorderPixels do
					if pixelData[v] == enemyId then
						colorPart(v, enemyBorderColor)
					end
				end
				break
			end -- if no losses, no attack happened
			newBorderPixels, newEnemyBorderPixels = expandPixels(playerId, enemyId, true)
			attackingTroops = remainingTroops

		end

		-- recolor borders at end
		--local borderColor = getBorderColor(playerNation.Color)
		--task.wait()
		for i,v in newBorderPixels do
			if pixelData[v] == playerId then
				colorPart(v, borderColor)

			end
		end
		for i,v in newEnemyBorderPixels do
			if pixelData[v] == enemyId then
				colorPart(v, enemyBorderColor)
			end
		end
	end)

end




function Actions.Boat(playerId, pixelValue, pixelId)
	local playerNation = Nation.byPlayerId[playerId]
	local selectedNation = Nation.byPlayerId[pixelValue]
	local percent = getPercent(playerNation)

	-- broken check v
	--if playerNation:isAlly(pixelValue) and selectedNation.Team == playerNation.Team then
	--	return
	--end

	print("check boat conditions again, then send it fam!")

	-- get the path, get the troops, tween the color over the path
	warn("player id:", playerId, "pixel id:", pixelId)
	local prevPixel = nil
	local prevColor = nil
	local boatable, line, startPixel, endPixel = isBoatable(playerId, pixelId)
	if boatable == true then
		local waterStart, waterEnd = isAdjacentToWater(startPixel), isAdjacentToWater(endPixel)
		if waterStart and waterEnd then
			local waterLine = getLinePixels(waterStart, waterEnd)
			for i, pixelIndex in waterLine do


				-- sanity check
				--if pixelData[pixelIndex] ~= 1 then
				--	print(tostring(pixelIndex) .. " is not water")
				--	return false
				--end

				--set prevColor

				--print(partData[pixelIndex])



				-- color the PrevPixel to prevColor
				if prevPixel ~= nil then
					local part = partData.GetPart(prevPixel)
					part.Color = prevColor
				end

				prevColor = partData.GetColorOfIndex(pixelIndex)

				-- color the Part
				colorPart(pixelIndex, Color3.new(1, 1, 1))


				print("Setting prevPixel to: " .. tostring(pixelIndex))

				prevPixel = pixelIndex

				--colorPart(pixelIndex, Color3.new(1, 1, 1))
				--print(pixelIndex)

				task.wait(tickLength)

			end

			warn("deploying troops from boat")
			local returnNationId = pixelData[endPixel]

			-- Check density then create border pixel
			--if not (attackingTroops > (2 * Calculate.calculateBorderDensity(getTroopValue(enemy), getLandValue(enemy)))) then
			--	return
			--end

			-- convert targetPixel to attacker territory

			pixelData[endPixel] = playerId
			table.insert(playerNation.borderPixels, endPixel)


			if selectedNation then
				local player = game.Players:GetPlayerByUserId(pixelValue)
				local botOrPlayer = player or pixelValue
				warn("set land value")
				setLandValue(botOrPlayer, getLandValue(botOrPlayer) - 1)
			end

			-- INSERT BOAT COLOR FIX IN HERE
			task.spawn(function()
				task.wait(2 * tickLength)
				-- pixelData[endPixel] = 1
				-- local find = table.find(playerNation.borderPixels, endPixel)
				-- if find then
				--     --warn("REMOVE")
				--     table.remove(playerNation.borderPixels, find)
				-- end

				if selectedNation then
					local find2 = table.find(selectedNation.borderPixels, endPixel)
					if find2 then
						--warn("REMOVE")
						table.remove(selectedNation.borderPixels, find2)
					end
				end
				if prevPixel ~= nil then
					local part = partData.GetPart(prevPixel)
					part.Color = prevColor
				end
			end)

			--expandPixels(playerId, returnNationId, true)

			--attackingTroops -= 2 * Calculate.calculateBorderDensity(getTroopValue(enemy), getLandValue(enemy))

			return returnNationId
		end
	end
end




-- THIS SHOULD BE HANDLED FROM THE SERVER, SO WE WILL DO THE UI PROMPT FROM HERE. ALL OTHER UI NOTIFICATIONS RUN THROUGH CLIENT SCRIPT GUIHANDLER, AND GETS REMOTES FROM THIS ACTIONS MODULE
function Actions.Ally(playerId, pixelValue)
	local playerNation = Nation.byPlayerId[playerId]
	local selectedNation = Nation.byPlayerId[pixelValue]

	-- Check if the selected nation is not the same team and not a bot
	if not (selectedNation.Team == playerNation.Team and not selectedNation.isBot) then
		local recievingPlayer = game.Players:GetPlayerByUserId(selectedNation.playerId)
		local requestingPlayer = game.Players:GetPlayerByUserId(playerNation.playerId)
		-- Trigger the client to handle the ally request UI
		ReNotify(recievingPlayer, playerNation.Name .. " has sent you an ally request.", {
			Options = {
				{
					Text = "Accept",
					Primary = true,
				},
				{
					Text = "Deny",
					Primary = false,
				}
			},
			Timeout = 10;

			Callback = function(choice)
				--print(choice)
				if choice == 1 then
					-- Ally if the response is Yes
					print("Accepting Ally!")
					table.insert(playerNation.Allies, selectedNation.playerId)
					table.insert(selectedNation.Allies, playerNation.playerId)
					ReNotify(requestingPlayer, tostring(selectedNation.Name .. " has accepted your ally request."),
						{
							Timeout = 5;
						}
					)
					local player1 = game.Players:GetPlayerByUserId(selectedNation.playerId)
					local player2 = game.Players:GetPlayerByUserId(playerNation.playerId)
					local remote1 = player1:FindFirstChild("RemoteEvent")
					local remote2 = player2:FindFirstChild("RemoteEvent")
					remote1:FireClient(player1, playerNation.playerId, true)
					remote2:FireClient(player2, selectedNation.playerId, true)
				else
					print("Denying Ally!")
				end
			end
		})
	end
end


function Actions.Donate(playerId, pixelValue)
	local playerNation = Nation.byPlayerId[playerId]
	local selectedNation = Nation.byPlayerId[pixelValue]
	if not playerNation:isAlly(pixelValue) and selectedNation.Team ~= playerNation.Team then
		return
	end
	local percent = getPercent(playerNation)

	local playerSending = playerId
	local playerRecieving = pixelValue
	if playerNation.isBot == false then
		playerSending = game.Players:GetPlayerByUserId(playerId)
	end
	if selectedNation.isBot == false then
		playerRecieving = game.Players:GetPlayerByUserId(pixelValue)
	end

	local troopsToAdd = math.floor(getTroopValue(playerSending) * percent + .5)

	print(troopsToAdd)

	updateTroopValue(playerSending, getTroopValue(playerSending) - troopsToAdd)

	updateTroopValue(playerRecieving, getTroopValue(playerRecieving) + troopsToAdd)

	ReNotify(playerSending, tostring(" You sent " .. troopsToAdd .. " troops to " .. selectedNation.Name .. "."),
		{
			Timeout = 5;
		}
	)

	ReNotify(playerRecieving, tostring(playerNation.Name .. " has sent you ".. troopsToAdd .. " troops."),
		{
			Timeout = 5;
		}
	)

	-- make sure you cant donate over cap limit of nation!

	-- DISPLAY IN GUIHANDLER

	-- CALCULATE
end

return Actions

