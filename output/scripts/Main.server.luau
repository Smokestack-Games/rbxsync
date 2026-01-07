-- Services and required modules
local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local DataStoreService = game:GetService("DataStoreService")

local Nation = require(ReplicatedStorage.Classes.Nation)
local InitModule = require(script.Init)
local Actions
local Calculate = require(script.Parent.ActionHandler.Calculate)
local ReNotify = require(game.ServerScriptService.MainModule)  -- Assuming this is the correct module ID
local Actions

-- Main Game Class
local Game = {}
Game.__index = Game

local botTotal = 95 --270

function Game.new()
	local self = setmetatable({}, Game)
	self.gameStatus = ReplicatedStorage:WaitForChild("Data").GameStatus.Value
	self.tickLength = game.ReplicatedStorage.Data.TickLength.Value
	self.ticksToSpawn = game.ReplicatedStorage.Data.TicksToSpawn.Value
	self.loadMapGate = true
	self:setupEvents()
	return self
end

-- Load player data from datastore
function Game:loadData(player)
	-- Load player name and color from datastore
	local PlayerStore = DataStoreService:GetDataStore("PlayerPreferences")
	local data = PlayerStore:GetAsync(player.UserId) or {}
	return {
		name = data.Name or player.Name,
		color = (data.Color and typeof(data.Color) == "table" and #data.Color == 3) and Color3.new(unpack(data.Color)) or Color3.new(1,1,1),
		flag = data.Flag,
		skin = data.Skin
	}
end


-- Load test data (used in roblox studio)
function Game:loadTestData()
	self:loadGame("Desert", "ZOMBOT")
end



-- Asynchronously load the game mode and map
function Game:loadGame(mapName, gameMode)
	spawn(function()
		InitModule:LoadGameMode(gameMode)
		InitModule:LoadMap(mapName)
		-- spawm 40 bots
		-- Nation needs to select spawn points, or do it from here? 

		Actions = require(script.Parent.ActionHandler.Actions)

		Nation:createBots(botTotal)
		local mapDataTemp = require(ReplicatedStorage:WaitForChild("MapData"))
		local indexTotal = mapDataTemp.width * mapDataTemp.height

		local debounce = true
		while debounce do
			wait(1)

			if game.ReplicatedStorage.Data.LoadProgress.Value > 62000 then
				debounce = false
			end
		end

		-- spawn the bots in world
		for i=1, botTotal do
			-- spawn needs to validate
			Actions.Spawn(i+2, math.floor((indexTotal/botTotal) * i))
			--			print("spawning " .. "Bot" .. i+2 .. " at " .. math.floor((indexTotal/botTotal) * i))

		end



		self.gameStatus = 1
		ReplicatedStorage:WaitForChild("Data").GameStatus.Value = 1
	end)
end


-- Initialize map data from the first player
function Game:initMapDataFromFirstPlayer(player)
	-- Set up map and mode from first player's teleport data
	local joinData = player:GetJoinData()
	if not joinData.TeleportData then
		self:loadTestData() -- Test in Roblox Studio
		return
	end
	local mapName = joinData.TeleportData.mapKey
	local gameMode = joinData.TeleportData.gameMode
	self:loadGame(mapName, gameMode)
end


local baseColors = {
	Red = Color3.new(1, 0, 0),
	Blue = Color3.new(0, 0, 1),
	Green = Color3.new(0, 1, 0),
	Yellow = Color3.new(1, 1, 0),
	Pink = Color3.new(1, 0, 1),
	Cyan = Color3.new(0, 1, 1),
	Black = Color3.new(0.4, 0.4, 0.4),
	White = Color3.new(1, 1, 1)
}

local function shadeOf(color, factor)
	return Color3.new(math.clamp(color.r * factor, 0, 1),
		math.clamp(color.g * factor, 0, 1),
		math.clamp(color.b * factor, 0, 1))
end


-- ... [Previous definitions remain unchanged]
function Game:setupNationForPlayer(player, playerData)
	local gameMode = ReplicatedStorage.Data.Gamemode.Value
	local Teams = game.Teams

	if gameMode == "FFA" then
		for _, team in pairs(Teams:GetChildren()) do
			if team.Name ~= "Spectators" and team.Name ~= "Players" then
				--if team.Name ~= "Players" then
					team:Destroy()
				--end
			end
		end
		Nation.new(false, playerData.name, playerData.color, playerData.flag, playerData.skin, player.UserId, player.UserId)
		print(player.UserId)
		player.Team = Teams.Players

	elseif gameMode == "ZOMBOT" then
		local teamColor = shadeOf(baseColors.White, math.random() * 0.4 + 0.8)
		Nation.new(false, playerData.name, teamColor, playerData.flag, playerData.skin, player.UserId, 8)
		player.Team = Teams.White

		for _, team in pairs(Teams:GetChildren()) do
			if team.Name ~= "Black" and team.Name ~= "White" and team.Name ~= "Spectators" then
				team:Destroy()
			end
		end

	elseif gameMode:sub(-3) == "TDM" then
		local maxTeams = tonumber(gameMode:sub(1, 1))
		local teamNames = {"Red", "Blue", "Green", "Yellow", "Pink", "Cyan", "Black", "White"}
		local index = math.random(1, maxTeams)
		local selectedTeamName = teamNames[index]
		local teamColor = shadeOf(baseColors[selectedTeamName], math.random() *.5 + .5)

		player.Team = Teams[selectedTeamName]
		
		Nation.new(false, playerData.name, teamColor, playerData.flag, playerData.skin, player.UserId, index)
		
		local function isTeamValid(teamName)
			for i = 1, maxTeams do
				if teamNames[i] == teamName then
					return true
				end
			end
			return false
		end

		for _, team in pairs(Teams:GetChildren()) do
			if not isTeamValid(team.Name) and team.Name ~= "Spectators" then
				team:Destroy()
			end
		end

	else
		print("Could not parse gameMode string")
	end
end



-- Make the player a spectator
function Game:setupSpectator(player)
	local character = player.Character
	if not character then return end
	-- Make the character semi-transparent and disable collisions
	for _, part in pairs(character:GetDescendants()) do
		if part:IsA("BasePart") and part.Name ~= "HumanoidRootPart" then
			part.Transparency = 0.7
		end
	end
	character:FindFirstChildWhichIsA("Humanoid"):ChangeState(Enum.HumanoidStateType.Physics)
end

-- Handle player added event
function Game:playerAdded(player)
	if self.loadMapGate then
		self.loadMapGate = false
		self:initMapDataFromFirstPlayer(player)
	end
	while ReplicatedStorage.Data.Gamemode.Value == "" do
		wait()
	end
	if self.gameStatus < 2 then
		local playerData = self:loadData(player)
		self:setupNationForPlayer(player, playerData)
		print("SetupNation!!!")
		-- Change state of player to physics
		local Character = player.Character or player.CharacterAdded:Wait()
		Character:FindFirstChildWhichIsA("Humanoid"):ChangeState(Enum.HumanoidStateType.Physics)
	else
		self:setupSpectator(player) -- Set the player to be a spectator.
	end
end


function Game:calculateOccupancy()
	local biggestNationSize = ReplicatedStorage.Data.BiggestNationSize.Value
	local mapLandSize = ReplicatedStorage.Data.MapLandSize.Value

	local occupancy = biggestNationSize / mapLandSize

	return occupancy
end

local function getLandValue(player)
	local isBot = typeof(player) ~= "Instance"

	if isBot then
		local getBotData = game.ReplicatedStorage.BotData:FindFirstChild(tostring(player))
		if getBotData then
			return getBotData.Land.Value
		else
			return
		end
	else
		if player and player:FindFirstChild("leaderstats") then
			return player.leaderstats.Land.Value
		else
			return
		end
	end

end




local alivePlayerIds = {}
local deadPlayerIds = {}

local gameStart = false
local gameOver = false
local gameNotOver = false
local teamNames = {"Red", "Blue", "Green", "Yellow", "Pink", "Cyan", "Black", "White"}
-- handle player ticks
function Game:tickMath(gameTick)
	
	local gamemode = ReplicatedStorage.Data.Gamemode.Value
	
	if gameStart == false then
		for i,v in Nation.byPlayerId do
			table.insert(alivePlayerIds, v.playerId)
		end
		gameStart = true
		--print(alivePlayerIds)
	end
	--local remainingNations = difference(alivePlayerIds, deadPlayerIds)
	--print(remainingNations)
	-- add troops to all Nations
	--print(gamemode)
	if gamemode == "FFA" and gameOver == false then
		--print("isFFA")
		-- check to ensure all surviving nations are on the same ally!
		for i, v in ipairs(alivePlayerIds) do
			--print(v)
			if i > 1 then
				if not table.find(Nation.byPlayerId[alivePlayerIds[1]].Allies, v) then
					--break
					gameNotOver = true
					break
				else --if gameNotOver == false then
					if i == #alivePlayerIds then
						gameOver = true
						
						print("GAME OVER")
						self.gameStatus = 3
						ReplicatedStorage.Data.GameStatus.Value = 3
						ReplicatedStorage.Remotes.WinEvent:FireAllClients(tostring(Nation.byPlayerId[alivePlayerIds[1]].Name))
					end
				end
				gameNotOver = false
			elseif #alivePlayerIds == 1 then
				gameOver = true
				print("GAME OVER")
				self.gameStatus = 3
				ReplicatedStorage.Data.GameStatus.Value = 3
				ReplicatedStorage.Remotes.WinEvent:FireAllClients(tostring(Nation.byPlayerId[alivePlayerIds[1]].Name))
			end
			
		end
	elseif gameOver == false then -- TDM and zombot
		-- check to ensure all surviving nations are on the same team!
		for i, v in ipairs(alivePlayerIds) do
			if i > 1 then
				if Nation.byPlayerId[alivePlayerIds[1]].Team ~= Nation.byPlayerId[alivePlayerIds[i]].Team then
					
					gameNotOver = true
					break
				else --if gameNotOver == false then
					if i == #alivePlayerIds then
						gameOver = true
						print("GAME OVER")
						self.gameStatus = 3
						ReplicatedStorage.Data.GameStatus.Value = 3
						ReplicatedStorage.Remotes.WinEvent:FireAllClients(teamNames[Nation.byPlayerId[alivePlayerIds[1]].Team])
					end
				end
				gameNotOver = false
			elseif #alivePlayerIds == 1 then
				-- is last
				gameOver = true
				print("GAME OVER")
				self.gameStatus = 3
				ReplicatedStorage.Data.GameStatus.Value = 3
				ReplicatedStorage.Remotes.WinEvent:FireAllClients(teamNames[Nation.byPlayerId[alivePlayerIds[1]].Team])
			end

		end
	end
	
	for _, nation in pairs(Nation.byPlayerId) do
		local success, result = pcall(function()

			local nationId = nation.playerId
			local player = nation.playerId

			if nation.isBot == false then
				if game.Players:GetPlayerByUserId(player) then
					player = game.Players:GetPlayerByUserId(player)
				else
				--	return
					
				end
			end
			
			

			if getLandValue(player) == 0 and table.find(deadPlayerIds, nation.playerId) == nil then
				-- make sure it doesnt repeat this if its dead already
				-- table of dead playerIds
				table.remove(alivePlayerIds, table.find(alivePlayerIds, nation.playerId))
				table.insert(deadPlayerIds, nation.playerId)
				--print(deadPlayerIds)
			
				-- remove player tags
				for i,player in ipairs (game.Players:GetChildren()) do
					ReNotify(player, tostring(nation.Name .. " has fell."),
						{
							Type = "critical";
							Timeout = 2;
						}
					)
				end


				local tag = game.Workspace.Tags:FindFirstChild(nationId)
				if tag then
					tag:Destroy()
				end


				-- Check if the player with the given userId is in the game
				if typeof(player) == "Instance" then
					--local playerObj = game.Players:GetPlayerByUserId(player)

					-- Team the player to spectators (Assuming you have a Spectators team)
					--local spectatorsTeam = game.Teams:FindFirstChild("Spectators")
					player.Team = game:GetService("Teams")["Spectators"]


					-- Kill the player (assuming you have a function to do that)
					local function killPlayer(player)
						player.Character.Humanoid.Health = 0
						-- You should implement logic to handle player death here.
						-- For example, respawning the player or ending their game.
						-- This could involve setting their Humanoid's health to 0, etc.
					end

					-- Make the player half transparent


					-- Call the functions to kill and make the player half transparent
					killPlayer(player)
					local playerGui = player.PlayerGui
					playerGui.ExitGame.Window.Visible = true
					
					--makePlayerHalfTransparent(player)



				end

			end



			if gameTick % 10 == 0 then
				-- big boost
				Calculate.updateBalanceFromIncome(player)
			else
				-- regular interest boost
				Calculate.updateBalanceFromInterest(player)
			end

			Calculate.updateInterestRate(player,gameTick)
		end)
		
		if success == false then
			warn("GAME TICK ITERATION FAILED:", result)
		end
	end
end


-- handle bot ticks

-- use calculations! mirror how actionHandler handles it
function Game:botAI(botId)
	
	local botNation = Nation.byPlayerId[botId]
	if botNation == nil then
		warn("Bot nation couldn't be found with id:", botId, Nation.byPlayerId)
	end
	
	
	-- implement evaluation of troops and enemies to determine attack
	-- needs quick math to end the thread and return if it doesnt meet density requirements or something
	-- add in donation logic if its surrounded by friendlies
	-- add boat logic if surrounded by friendlies but also has water
	-- raycast randomly and if it can find place to go, send boat
	
	-- random agressiveness to determine what size enemy willing to attack
	
	
	if Calculate.botAction(botId) then
		-- Check for empty land (playerId "2")
		if botNation:SharesBordersWith(2) then
			-- 
			Actions.Expand(botId)

		else
			-- Attack a random neighbor if no empty land
			local randomEnemy, callback, pixel, pixelIndex = botNation:GetRandomEnemy()
			--warn("enemy:", randomEnemy, "pixel:", pixel, "pixel index:", pixelIndex)
			-- attack from random border
			if randomEnemy then
				Actions.Attack(botId, callback or function() return randomEnemy end, botId, pixel, pixelIndex)  -- Assuming you have an "Attack" function in actionsTemp
				--task.wait()
			end
		end
	end
end


-- CROWN UI

-- Function to find the player with the most land
function Game:findNationWithMostLand()
	local maxLand = 0
	local nationWithMostLand = nil

	for _, nation in pairs(Nation.byPlayerId) do
		if #nation.borderPixels > 0 then -- Ignore dead nations
			local landValue = 0
			if nation.isBot == true then
				-- It's a bot, get the land value from ReplicatedStorage.BotData
				landValue = game.ReplicatedStorage.BotData[tostring(nation.playerId)].Land.Value
			else
				-- It's a player, get the land value from leaderstats
				local player = game.Players:GetPlayerByUserId(nation.playerId)
				if player and player.leaderstats then
					landValue = player.leaderstats.Land.Value
				end
			end

			if landValue > maxLand then
				maxLand = landValue
				nationWithMostLand = nation.playerId
			end
		end
	end

	return nationWithMostLand
end


local RecentCrownTag = nil

local botGate = true
local gameTicks = 0
-- Main game loop
function Game:gameLoop()
	--Actions = require(script.Parent.ActionHandler.Actions)
	--task.spawn(function()
	--	while task.wait() do
	--		task.wait(self.tickLength)
	--		for playerId, _ in Nation.byPlayerId do
	--			local player = game.Players:GetPlayerByUserId(playerId)
	--			warn(playerId, "nation land:", Actions.getLandValue(player or playerId))
	--		end
	--	end
	--end)
	
	while task.wait(self.tickLength) do
		--local success, result = pcall(function()

			if self.gameStatus == 0 then
				--print("loading...")
			elseif self.gameStatus == 1 then
				--print("can spawn now... " .. self.ticksToSpawn)
				self.ticksToSpawn = self.ticksToSpawn - 1
				game.ReplicatedStorage.Data.TicksToSpawn.Value = game.ReplicatedStorage.Data.TicksToSpawn.Value - 1
				if self.ticksToSpawn < 1 then
					ReplicatedStorage:WaitForChild("Data").GameStatus.Value = 2
					self.gameStatus = 2
					print("successfully changed gamestatus")
				end
				print(self.ticksToSpawn)
			elseif self.gameStatus == 2 then
				-- add tick count
				ReplicatedStorage.Data.CurrentTick.Value = ReplicatedStorage.Data.CurrentTick.Value + 1 

				gameTicks += 1

				-- "Main loop" If 99% occupancy, shut it down
				if self:calculateOccupancy() >= 0.99 then
					ReplicatedStorage:WaitForChild("Data").GameStatus.Value = 3
					self.gameStatus = 3
				end

				-- add troops to all nations per tick
				self:tickMath(gameTicks)

				-- Calculate the player with the most land
				local nationWithMostLand = self:findNationWithMostLand()

				if nationWithMostLand then
					if RecentCrownTag ~= nil then
						RecentCrownTag.Visible = false
					end
					game.Workspace.Tags[nationWithMostLand].Crown.Visible = true
					RecentCrownTag = game.Workspace.Tags[nationWithMostLand].Crown
					--print("Player with the most land: " .. playerWithMostLand.Name)
				else
					--print("No player found with the most land.")
				end
				--if gameTicks % 10 == 1 then
				-- run bot AI every tick to determine if it wants to expand/attack or not

				for _, nation in pairs(Nation.byPlayerId) do
					if nation.isBot and #nation.borderPixels > 0 then
						-- need to check if bot exists

						-- run bot AI to check for actions from each bot every tick	
						self:botAI(nation.playerId)
					end
				end
				--end

				-- Add more game logic as necessary here.
			elseif self.gameStatus == 3 then
				print("round ended")
				-- Handle end-of-round logic, such as cleanup or setting up the next round.
			end
		--end)
		
		--if success == false then
		--	warn("GAME LOOP ITERATION FAILED:", result)
		--end
	end
	

end

-- Setup player added event
function Game:setupEvents()
	
	Players.PlayerAdded:Connect(function(player)
		player:LoadCharacter()
		self:playerAdded(player)
	end)
end

-- Run the game
local gameInstance = Game.new()
gameInstance:gameLoop()


-- set up map and mode from first player's teleportdata
-- set up pixel data (parent the MapData to serverscriptservice)



-- changes GameStatus as it loads, so gameServer can double check client inputs, and it limits clients from sending remotes when not allowed + informs client of game state

-- declare nations as people join
-- “Main loop” If 99% occupancy, shut it down
-- ASYNC Each tick after round starts, run addTroops
