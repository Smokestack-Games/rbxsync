local ServerStorage = game:GetService("ServerStorage")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Debris = game:GetService("Debris")

local Init = {}
Init.__index = Init



function Init:LoadMap(MapName)
	-- Find the map
	local map = ServerStorage.Maps:FindFirstChild(MapName)
	if not map then
		warn("Failed to find map with name: " .. MapName)
		return self
	end

	local mapData = require(map:FindFirstChild("MapData"))
	
	-- Asynchronous part creation
	local co = coroutine.create(function()
		local partData = require(ReplicatedStorage.PartData)
		partData.CreatePartData(map.voxels, mapData.width * mapData.height)
	end)

	-- Start the coroutine
	coroutine.resume(co)

	
	-- Cache references for quicker access
	local replicatedData = ReplicatedStorage.Data

	-- Set the map data in ReplicatedStorage.Data
	replicatedData.MapName.Value = MapName
	replicatedData.MapLandSize.Value = self:CalculateSizeFromPixelData(mapData.pixelData, 2)
	replicatedData.MapWaterSize.Value = self:CalculateSizeFromPixelData(mapData.pixelData, 1)
	replicatedData.MapTotalSize.Value = mapData.height * mapData.width

	-- Remove all other maps in ServerStorage.Maps
	for _, otherMap in ipairs(ServerStorage.Maps:GetChildren()) do
		if otherMap ~= map then
			Debris:AddItem(otherMap, 0)  -- Destroy immediately
		end
	end

	-- Parent voxel folder earlier for faster client loading
	map.voxels.Parent = game.Workspace

	-- Move map data to the correct locations
	map.MapData.Parent = ReplicatedStorage

	
	-- Wait until the coroutine completes
	while coroutine.status(co) ~= "dead" do
		task.wait()  -- You can adjust this wait time as needed
	end

	return self
end


-- Helper function to calculate size based on pixel data
function Init:CalculateSizeFromPixelData(pixelData, type)
	local count = 0
	for _, value in pairs(pixelData) do
		if value == type then
			count = count + 1
		end
	end
	return count
end

function Init:LoadGameMode(GamemodeName)
	ReplicatedStorage.Data.Gamemode.Value = GamemodeName
	return self
end

return Init
