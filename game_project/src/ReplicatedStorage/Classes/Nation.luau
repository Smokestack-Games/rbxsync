local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local ServerScriptService = game:GetService("ServerScriptService")
local ServerStorage = game:GetService("ServerStorage")
local PartData = require(ReplicatedStorage.PartData)

local Nation = {}
Nation.__index = Nation

local mapDataModule = nil
local mapData = nil
local width = nil
local height = nil
local pixelData = nil

game.ReplicatedStorage.ChildAdded:Connect(function(child)
	if child.Name == "MapData" then
		mapDataModule = child
		mapData = require(mapDataModule)
		width = mapData.width
		height = mapData.height
		pixelData = mapData.pixelData
	end
end)

-- Dictionary to lookup nations by playerId
Nation.byPlayerId = {}




local count = 3

-- Leaderboard setup for a nation (either a player or a bot)
function Nation:leaderboardSetup(isBot, playerId)
	local container
	if isBot then
		container = ReplicatedStorage
	else
		container = Players:GetPlayerByUserId(playerId)
	end
	-- isBot and ReplicatedStorage or Players:FindFirstChild(self.Name)
	local leaderstats = Instance.new("Folder")
	self.Leaderstats = leaderstats
	leaderstats.Name = "leaderstats"

	if isBot then
		local botFolder = Instance.new("Folder")
		botFolder.Name = count
		count = count + 1
		botFolder.Parent = ReplicatedStorage.BotData
		warn("Bot folder created for", playerId)
		leaderstats = botFolder
	else 
		leaderstats.Parent = container
		print(container.Name)
	end

	local econ = Instance.new("IntValue")
	econ.Name = "Troops"
	econ.Value = 16 -- Set the initial Troops value as needed
	econ.Parent = leaderstats

	local land = Instance.new("IntValue")
	land.Name = "Land"
	if isBot then
		land.Value = 1
	else
		land.Value = 0
	end
	-- Set the initial Land value as needed
	land.Parent = leaderstats

	local interest = Instance.new("NumberValue")
	interest.Name = "Interest"
	interest.Value = .07 -- Set the initial Interest value as needed
	interest.Parent = leaderstats

	--	print("Set up leaderboard for: " .. self.Name)
end


function Nation.new(isBot, name, color, flag, skin, playerId, team)
	local self = setmetatable({}, Nation)
	self.isBot = isBot
	if name == nil or name == "" then
		self.Name = game.Players:GetPlayerByUserId(playerId).Name
	else
		self.Name = name
	end
	self.Color = color
	self.Flag = flag
	self.Skin = skin
	-- amount of total pixels
	--self.pixelsOwned = 0
	-- list of part indexes
	self.borderPixels = {}
	-- list of part indexes
	self.fillPartFolder = {}

	-- initial troops
	-- could do 128
	--self.Troops = 512
	self.playerId = playerId
	-- list of allies
	self.Allies = {}

	--self.Interest = .07
	-- do not use roblox teams, we want the leaderboard to sort all player nations by default. this is only used for our distinctions
	-- use a number to identify the team (1-8)
	self.Team = team

	if not isBot then
		print("Setting up leaderboard")
	end
	-- Leaderboard setup for nation, be it a player or a bot
	self:leaderboardSetup(isBot, playerId)

	Nation.byPlayerId[playerId] = self

	return self
end

-- This is a table representing base colors for each team
local baseColors = {
	[1] = Color3.new(1, 0, 0),  -- red for team 1 (Teams.Red)
	[2] = Color3.new(0, 0, 1),  -- blue for team 2 (Teams.Blue)
	[3] = Color3.new(0, 1, 0),  -- green for team 3 (Teams.Green)
	[4] = Color3.new(1, 1, 0),  -- yellow for team 4 (Teams.Yellow)
	[5] = Color3.new(1, 0, 1),  -- pink for team 5 (Teams.Pink)
	[6] = Color3.new(0, 1, 1),  -- cyan for team 6 (Teams.Cyan)
	[7] = Color3.new(0.4, 0.4, 0.4),  -- black for team 7 (Teams.Black)
	[8] = Color3.new(1, 1, 1),  -- white for team 8 (Teams.White)
}


-- Function to generate a shade of a given color
-- The factor will determine the shade, with 1 being the original color
local function shadeOf(color, factor)
	return Color3.new(math.clamp(color.r * factor, 0, 1),
		math.clamp(color.g * factor, 0, 1),
		math.clamp(color.b * factor, 0, 1))
end




-- List of real country names
local countryNames = {"Afghanistan", "Brazil", "China", "Denmark", "Egypt", "France", "Germany", "Hungary", 
	"India", "Japan", "Kazakhstan", "Libya", "Mexico", "Nigeria", "Oman", "Poland", "Qatar", 
	"Russia", "Spain", "Turkey", "Ukraine", "Venezuela", "Yemen", "Zimbabwe"}

-- List of prefixes and affixes
local prefixes = {"Great", "New", "United", "South", "North", "East", "West"}
local affixes = {"Republic", "Kingdom", "Empire", "Federation", "Commonwealth", "Dominion", "State"}

-- Function to generate a random country name
local function GenerateCountryName()
	local prefix = prefixes[math.random(#prefixes)]
	local countryName = countryNames[math.random(#countryNames)]
	local affix = affixes[math.random(#affixes)]

	-- Randomly decide to add a prefix, an affix, or both
	local styleOptions = {"prefix", "affix", "both"}
	local nameStyle = styleOptions[math.random(#styleOptions)]

	if nameStyle == "prefix" then
		return prefix .. " " .. countryName
	elseif nameStyle == "affix" then
		return countryName .. " " .. affix
	else
		return prefix .. " " .. countryName .. " " .. affix
	end
end




-- Function to create and set up bots
function Nation:createBots(numBots)
	local gameMode = ReplicatedStorage.Data.Gamemode.Value
	if gameMode == "FFA" then
		for i = 1, numBots do
			local botName = GenerateCountryName()
			-- BOTID IS >2
			Nation.new(true, botName, Color3.new(math.random(), math.random(), math.random()), "", "", i+2, i+2)

		end
	elseif gameMode == "ZOMBOT" then
		for i = 1, numBots do
			local botName = GenerateCountryName()
			-- BOTID IS >2
			Nation.new(true, botName, shadeOf(baseColors[7], math.random() * 0.4 + 0.8), "", "", i+2, 7)
		end
	elseif string.sub(gameMode, -3) == "TDM" then
		for i = 1, numBots do
			local botName = GenerateCountryName()
			-- BOTID IS >2

			local teamId = math.random(1, tonumber(gameMode:sub(1, 1)))
			local teamColor = shadeOf(baseColors[teamId], math.random() *.5 + .5)


			Nation.new(true,botName, teamColor, "", "", i+2, teamId)

		end
	else
		print("Could not parse gameMode string")
	end



end

-- Add functions to calculate interest and troop gain
function Nation:CalculateInterest()
	local interest = .07
	return interest
end


local function getNeighborPixels(pixelId)
	local neighborPixels = {}
	-- Add right neighbor if not on the right border
	if pixelId % width ~= 0 then
		table.insert(neighborPixels, pixelId + 1)
	end
	-- Add left neighbor if not on the left border
	if pixelId % width ~= 1 then
		table.insert(neighborPixels, pixelId - 1)
	end
	-- Add bottom neighbor if not on the bottom border
	if pixelId <= width * (height - 1) then
		table.insert(neighborPixels, pixelId + width)
	end
	-- Add top neighbor if not on the top border
	if pixelId > width then
		table.insert(neighborPixels, pixelId - width)
	end
	return neighborPixels
end




function Nation:SharesBordersWith(playerId)
	for _, pixelId in ipairs(self.borderPixels) do
		local neighborPixels = getNeighborPixels(pixelId)

		for _, neighborPixel in ipairs(neighborPixels) do
			if pixelData[neighborPixel] == playerId and pixelData[pixelId] ~= playerId then
				--print(tostring(pixelData[pixelId]))
				return true, pixelId
			end
		end
	end
	return false
end

-- Get random enemy on land. If no enemies exist on land, then get random enemy from another land piece within boating line of sight.
function Nation:GetRandomEnemy()
	for _, pixelId in self.borderPixels do
		local randomBorderIndex = self.borderPixels[math.random(1, #self.borderPixels)]

		local neighborPixels = getNeighborPixels(randomBorderIndex)

		for _, neighborPixel in neighborPixels do
			if pixelData[neighborPixel] > 2 and pixelData[neighborPixel] ~= self.playerId and self.Team ~= Nation.byPlayerId[pixelData[neighborPixel]].Team then
				return pixelData[neighborPixel]
			end
		end
	end
	-- local Actions = require(ServerScriptService.ActionHandler.Actions)
	-- for _, nation in Nation.byPlayerId do
	--     if nation == self then continue end
	--     local sharesBorderWithWater, borderPixelId = nation:SharesBordersWith(1)
	--     if sharesBorderWithWater == false then continue end
	--     local c = 0
	--     warn("player id:", nation.playerId, "border pixel id:", borderPixelId, "pixelData:", pixelData[borderPixelId])
	--     if Actions.isBoatable(self.playerId, borderPixelId) then
	--         return pixelData[borderPixelId], Actions.Boat, pixelData[borderPixelId], borderPixelId
	--     end
	--     for _, pixelId in nation.borderPixels do
	--         if c % 5 == 0 then
	--             if Actions.isBoatable(self.playerId, pixelId) then
	--                 return pixelData[pixelId], Actions.Boat, pixelData[pixelId], pixelId
	--             end
	--         end
	--         c += 1
	--     end

	-- end
	return false
end

--function Nation:GetRandomEnemy()
--	for _, pixelId in ipairs(self.borderPixels) do
--		local randomBorderIndex = self.borderPixels[math.random(1, #self.borderPixels)]

--		local neighborPixels = getNeighborPixels(randomBorderIndex)

--		for _, neighborPixel in ipairs(neighborPixels) do
--			if pixelData[neighborPixel] > 2 and pixelData[neighborPixel] ~= self.playerId and self.Team ~= Nation.byPlayerId[pixelData[neighborPixel]].Team then
--				return pixelData[neighborPixel]
--			end
--		end
--	end
--	return false
--end

function Nation:isAlly(playerId)
	return table.find(self.Allies, playerId) ~= nil
end

function Nation:Destroy()

end

-- Event listener for players joining the game
Players.PlayerAdded:Connect(function(player)
	--Nation.new(false, player.Name, Color3.new(1, 1, 1), "", "", player.UserId, 1)  -- Modify attributes as needed
	-- create UI RemoteEvent here
	local event = Instance.new("RemoteEvent")
	event.Parent = player
	print("Player joined: " .. player.Name)
end)

Players.PlayerRemoving:Connect(function(player)
	local id = player.UserId

	local getNation = Nation.byPlayerId[id]

	--local storeCount = count
	getNation.isBot = true

	-- Replace leaderstats
	--getNation:leaderboardSetup(true, storeCount)
	local leaderstats = player:FindFirstChild("leaderstats")
	leaderstats.Name = id
	leaderstats.Parent = ReplicatedStorage.BotData
	--count += 1

	local tag = game.Workspace.Tags:FindFirstChild(id)
	if tag then
		local txt = tag.PlayerName.Text
		tag.PlayerName.Text = "<u>" .. txt .. "</u>"
	end

	--Nation.byPlayerId[player.UserId] = nil
	--Nation.byPlayerId[tostring(player)] = nil
	--Nation.byPlayerId[storeCount] = getNation
end)

return Nation
