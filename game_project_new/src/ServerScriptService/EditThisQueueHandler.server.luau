
-- ServerScript

local RemoteService = game:GetService("ReplicatedStorage")
local TeleportService = game:GetService("TeleportService")

local PlayerJoinQueue = RemoteService:WaitForChild("PlayerJoinQueue")
local PlayerLeaveQueue = RemoteService:WaitForChild("PlayerLeaveQueue")
local UpdateQueueInfo = RemoteService:WaitForChild("UpdateQueueInfo")

local queues = {}

local gameServerId = 15149408430

for i = 1, 6 do
	queues[i] = {players = {}, timeLeft = 60, gameMode = "", map = ""}
end


-- Sample table for game modes and maps
local gameModes = {"FFA", "2 TDM", "4 TDM", "ZOMBOT"}
local maps = {
	Lava = "rbxassetid://15041593671", -- lava
	Ice = "rbxassetid://15041593473", -- ice
	Amazon = "rbxassetid://15041593214", -- jungle
	Desert = "rbxassetid://15041593059", -- desert
}

-- Updated randomizeQueue function
local function randomizeQueue(queue)
	queue.gameMode = gameModes[math.random(#gameModes)]

	local mapKeys = {} -- List of map keys
	for key in pairs(maps) do
		table.insert(mapKeys, key)
	end

	queue.mapKey = mapKeys[math.random(#mapKeys)] -- Assign a random map key to the queue
	queue.map = maps[queue.mapKey] -- Assign the asset ID using the map key
end


local function teleportPlayers(queue)
	local playerArray = queue.players
	
	local teleportData = {
		mapKey = tostring(queue.mapKey),
		gameMode = queue.gameMode
	}

	local ServerStorage = game:GetService("ServerStorage")
	
		
	-- Use TeleportPartyAsync to send all players in the party to the same reserved server
	local success, errorMessage = pcall(function()
		
		
		--for i=1, #playerArray do
		--	print(playerArray[i])
		--end
		local reservedId = TeleportService:ReserveServer(gameServerId)

		TeleportService:TeleportToPrivateServer(15149408430, reservedId, playerArray, nil, teleportData, ServerStorage.LoadingScreen)
	end)

	if not success then
		warn("Failed to teleport players: " .. errorMessage)
	end
end


local function updateQueueInfo()
	local data = {}
	for _, queue in ipairs(queues) do
		table.insert(data, {#queue.players, queue.timeLeft, queue.map, queue.gameMode})
	end
	UpdateQueueInfo:FireAllClients(data)
end

PlayerJoinQueue.OnServerInvoke = function(player, queueNumber)
	table.insert(queues[queueNumber].players, player)
	updateQueueInfo()
end

PlayerLeaveQueue.OnServerInvoke = function(player, queueNumber)
	for i, p in ipairs(queues[queueNumber].players) do
		if p == player then
			table.remove(queues[queueNumber].players, i)
			break
		end
	end
	updateQueueInfo()
end

for i=1, 6 do
	randomizeQueue(queues[i])
	queues[i].timeLeft = i*10
	queues[i].players = {}
end

while wait(1) do
	for _, queue in ipairs(queues) do
		--if #queue.players > 0 then
			queue.timeLeft = queue.timeLeft - 1
			if queue.timeLeft <= 0 then
				task.spawn(function()
					teleportPlayers(queue)
				end)
				randomizeQueue(queue)
				queue.timeLeft = 60
				queue.players = {}
			end
		--end
	end
	updateQueueInfo()
end
