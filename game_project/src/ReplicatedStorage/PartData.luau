local PartData = {}



local partTable = nil

local loadProgress = game.ReplicatedStorage.Data.LoadProgress

local ROW_SIZE = 250
local MILLISECOND = 1/1000



function PartData.CreatePartData(partFolder, partCount)
	local partTableLocal = table.create(partCount)
	local partFolderChildren = partFolder:GetChildren()

	
	for i, part in pairs(partFolderChildren) do

		partTableLocal[i] = part

		if i % 250 == 1 then
			--print("Generated row")
			task.wait() 
		end
		

		loadProgress.Value = loadProgress.Value + 1
	end

	
	table.sort(partTableLocal, function(a, b)
		a, b = tonumber(a.Name), tonumber(b.Name)
		if a == nil then
			return false
		elseif b == nil then
			return true
		end
		return a < b
	end)

	print(partTableLocal)
	
	print("created part data")

	partTable = partTableLocal
end


function PartData.GetPart(partIndex)
	local part = partTable[partIndex]
	return part
end

local plastic = Enum.Material.SmoothPlastic

function PartData.ColorPart(partIndex, Color3)
	local part = partTable[partIndex]
	part.Color = Color3
	part.Material = plastic
end

function PartData.TexturePart(partIndex, enumMaterial)
	local part = partTable[partIndex]
	part.Material = enumMaterial
end

function PartData.GetColorOfIndex(partIndex)
	return partTable[partIndex].Color	
end

function PartData.GetTextureOfIndex(partIndex)
	return partTable[partIndex].Material	
end

return PartData
