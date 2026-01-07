-- This script should handle all client inputs, while Main handles the loop and informs this script of the game state (spawning, active, ended)

-- THIS SCRIPT IS THE ONLY ONE EDITING MAPDATA!! Main will parent it to serverscriptservice, and the Action module will require it. It requires when the GameStatus changes.

local ReplicatedStorage = game:GetService("ReplicatedStorage")
local ButtonEvent = ReplicatedStorage.Remotes.ButtonEvent
local TerritorySelectEvent = ReplicatedStorage.Remotes.TerritorySelectEvent
local GetPixelTypeFunction = ReplicatedStorage.Remotes.GetPixelTypeFunction

local mapDataModule = nil
local mapData = nil

game.ReplicatedStorage.ChildAdded:Connect(function(child)
	if child.Name == "MapData" then
		mapDataModule = child
		mapData = require(mapDataModule)
	end
end)


local actionModule = script:WaitForChild("Actions")
local Actions = require(actionModule)


local width = mapData.width --or 250
local height = mapData.height --or 250


function getIndexFromPosition(x, y)
	return x * 250 + y + 1
end


function getPixelIndexFromMouseHit(position)
	local roundedX = math.floor(position.x + 0.5)
	local roundedY = math.floor(position.z + 0.5)

	if roundedX < 0 or roundedX > width or roundedY < 0 or roundedY > height then
		return 0 -- Out of map bounds
	end

	-- Calculate pixel ID using the helper function
	local pixelID = getIndexFromPosition(roundedX, roundedY)
	return pixelID
end

function getPixelFromMouseHit(position)
	-- Round the position to nearest whole number
	local roundedX = math.floor(position.x + 0.5)
	local roundedY = math.floor(position.z + 0.5)

	if roundedX < 0 or roundedX > width or roundedY < 0 or roundedY > height then
		return 0 -- Out of map bounds
	end

	-- Calculate pixel ID using the helper function
	local pixelID = getIndexFromPosition(roundedX, roundedY)

	-- Return the pixel type from pixelData using pixelID
	return mapData.pixelData[pixelID] or 0 -- Return 0 if the pixelID doesn't exist for some reason

end


-- This returns pixelType so the client can display appropriate action buttons
GetPixelTypeFunction.OnServerInvoke = function(player, position)
	return getPixelFromMouseHit(position)
end


	
	
TerritorySelectEvent.OnServerInvoke = function(client, player, pixelType, position)
	local pixel = getPixelIndexFromMouseHit(position)
	return Actions.CheckTerritoryButtons(player, pixelType, pixel)
end




-- HANDLES TROOPS, NOT LAND
-- after an action runs, the player's Land will be updated

-- runs calculations for actions
-- runs actions based on the calculation results
-- send remotes to clients to alert of incoming attacks, ally requests, ally severances, and donations
-- set the troop values after each expansion and update the client UI (sent troops feed and nationTags) with the new troop count, the leaderstats will automatically update
-- remember sent troops is separate from your troop count, and they get readded if they arent all killed off

local Calculate = require(script.Calculate)

-- This script executes button actions assuming they are valid (and limit actions per tick)
ButtonEvent.OnServerEvent:Connect(function(player, buttonName, position)
	if buttonName == "Spawn" then
		-- Handle Spawn button logic
		-- should verify if its valid spawn
		
		print("Spawn action by", player.Name)
		
		if getPixelFromMouseHit(position) == 2 and ReplicatedStorage.Data.GameStatus.Value == 1 then
			Actions.Spawn(player.UserId, getPixelIndexFromMouseHit(position))
		end
		
	elseif buttonName == "Expand" then
		-- Handle Expand button logic
		
		print("Expand action by", player.Name)
		if getPixelFromMouseHit(position) == 2 then
			
			-- THIS IS FIRED BY PLAYER, BOTTICK WILL USE DIFFERENT CALCULATIONS
			-- calculate
	
			-- Calculations for expanding ran inside Actions module
			-- Actions module will send remotes to expanding client to show troops as it expands each expansion
			
			Actions.Expand(player.UserId)
			
			-- Update clients with the new troop count and territory
			-- Send any necessary remote events to clients

		end
		
		
	-- THE BELOW NEED VERIFICATION AS WELL	
	elseif buttonName == "Attack" then
		-- Handle Attack button logic
		
		print("Attack action by", player.Name)
		
		
			-- THIS IS FIRED BY PLAYER, BOTTICK WILL USE DIFFERENT CALCULATIONS
			-- calculate

			-- then expand 1
			Actions.Attack(player.UserId, getPixelFromMouseHit, position)

			-- update clients

			-- if remaining troops from calculate, attack until ended
	elseif buttonName == "Boat" then
		-- Handle Ally button logic

		print("Boat action by", player.Name)
		task.spawn(function()
			local pixel, pixelIndex = getPixelFromMouseHit(position), getPixelIndexFromMouseHit(position)
			local useCallback
			if pixel == 2 then
				useCallback = Actions.Expand
			else
				useCallback = Actions.Attack
			end
			
			useCallback(player.UserId, Actions.Boat, player.UserId, pixel, pixelIndex)
		end)


	elseif buttonName == "Ally" then
		-- Handle Ally button logic
		
		print("Ally action by", player.Name)
		
		Actions.Ally(player.UserId, getPixelFromMouseHit(position))
	elseif buttonName == "Donate" then
		-- Handle Donate button logic
		
		print("Donate action by", player.Name)
		Actions.Donate(player.UserId, getPixelFromMouseHit(position))
	end
end)
