--!strict
-- Cardboard is the UI handler for NxEngine2; It is responsible for using the existing assets to create a UI for the game.
-- Otherwise, it can be used for editing existing UIs by checking for a '.cb' module inside the assets which will include the UI structure.
-- This is the replacement of NanoWorks, which is Nano's UI handler.

-- Template for .cb modules;
--[[
    {
        [Property] = {Table of expected instances, Property, DefaultValue, ExpectedType?};
    }
]]

-- Example for a .cb module (Hint module);
--[[
return {
	["Title"] = {
		{
			script.Parent.Title;
		};
		"Text";
		"This is a title.";
		"string";
	};
	["Content"] = {
		{
			script.Parent.Content;
		};
		"Text";
		"This is some content.";
		"string";
	};
}
]] --> This is the expected structure for a .cb module. It is a table with the property names as keys, and the values are tables which follow the structure of {Instances, Property, DefaultValue and ExpectedType}.

-- If one does not exist, it will be attempted to be created locally.

local Cardboard = {};

local Players = game:GetService("Players");
local UserInputService = game:GetService("UserInputService");
local RunService = game:GetService("RunService");
local TweenService = game:GetService("TweenService");
local HttpService = game:GetService("HttpService"); -- Used for JSON encoding/decoding.
local Assets = script:FindFirstAncestorWhichIsA("ScreenGui"):FindFirstChild("Assets");

local LocalPlayer = Players.LocalPlayer;
local Mouse = LocalPlayer:GetMouse();

local AssetProperties = {}; -- Cache for .cb modules so they won't have to be loaded again.
local CreatedAssets = {}; -- Cache for assets that have been created; Mainly to store used UUIDs.

local function CreateUID()
	local UID = "";
	for i = 1, math.random(24, 32) do
		UID = UID .. string.char(math.random(65, 90));
	end
	return UID;
end

-- Cardboard( AssetType: string, Parent: Instance, [Properties: table] ); Returns: Asset <Instance>, UUID <string>
function Cardboard.new(AssetType, Parent, Properties)
	-- Generate a UUID for the asset.
	local UUID = CreateUID();
	
	if not Properties then Properties = {}; end;

	if not Assets:FindFirstChild(AssetType) then
		error("AssetType '"..AssetType.."' does not exist in the Assets folder. Does it exist?");
	end;

	-- Create the asset.
	local Asset = Assets[AssetType]:Clone();
	Asset.Parent = Parent;

	-- Structure the asset according to the .cb module.
	local cb = nil;

	if AssetProperties[UUID] then
		cb = AssetProperties[UUID];
	elseif Asset:FindFirstChild(".cb") then
		local cbModule = Asset:FindFirstChild(".cb");
		if cbModule and cbModule:IsA("ModuleScript") then
			cb = require(cbModule) :: {[any?]:any?};
			AssetProperties[UUID] = cb;
		end;
	end;

	if cb then
		for Key:string, Data:any in pairs(cb) do
			local Instances = Data[1];
			local Property = Data[2];
			local DefaultValue = Data[3];
			local ExpectedType = Data[4];

			if not Properties[Key] then
				Properties[Key] = DefaultValue;
			end;

			if ExpectedType and string.lower(typeof(Properties[Key])) ~= string.lower(ExpectedType) then
				error("Expected type '"..ExpectedType.."' for property '"..Key.."', got '"..typeof(Properties[Key]).."' instead.");
			end;

			for _, Instance in pairs(Instances) do
				local s,f = pcall(function()
					Instance[Property] = Properties[Key];
				end);
				if not s then
					warn("Failed to set property '"..Key.."' to '"..tostring(Properties[Key]).."' for instance '"..Instance.Name.."'.");
				end;
			end;
		end;
	end;
	
	-- Add the asset into the CreatedAssets table.
	CreatedAssets[UUID] = {Asset, AssetType};
	
	-- Make the asset visible.
	Asset.Visible = true;

	-- Return the asset and the UUID.
	return Asset, UUID;
end;

-- Cardboard:FindAsset( UUID: string ); Returns: StoredAsset <table{Asset, AssetType}>
function Cardboard:FindAsset(UUID)
	return CreatedAssets[UUID] or {nil, ""};
end;

-- Cardboard:UpdateAsset( UUID: string, NewProperties: table ); Returns: Success <boolean>
function Cardboard:UpdateAsset(UUID, NewProperties)
	local StoredAsset = self:FindAsset(UUID);
	local Asset = StoredAsset[1];
	local AssetType = StoredAsset[2];

	if not Asset then
		return false;
	end;

	local cb = nil;
	if AssetProperties[UUID] then
		cb = AssetProperties[UUID];
	else
		local cbModule = Asset:FindFirstChild(".cb");
		if cbModule and cbModule:IsA("ModuleScript") then
			cb = require(cbModule) :: ModuleScript;
			AssetProperties[UUID] = cb;
		end;
	end;

	if cb then
		for Key:string, Data:any in pairs(cb) do
			local Instances = Data[1];
			local Property = Data[2];
			local DefaultValue = Data[3];
			local ExpectedType = Data[4];

			-- Only change the edited values.
			if NewProperties[Key] then
				if ExpectedType and string.lower(typeof(NewProperties[Key])) ~= string.lower(ExpectedType) then
					error("Expected type '"..ExpectedType.."' for property '"..Key.."', got '"..typeof(NewProperties[Key]).."' instead.");
				end;

				for _, Instance in pairs(Instances) do
					local s,f = pcall(function()
						Instance[Property] = NewProperties[Key];
					end);
					if not s then
						warn("Failed to set property '"..Key.."' to '"..tostring(NewProperties[Key]).."' for instance '"..Instance.Name.."'.");
					end;
				end;
			end;
		end;
	end;

	return true;
end;

-- Cardboard:DestroyAsset( UUID: string ); Returns: Success <boolean>
function Cardboard:DestroyAsset(UUID)
	local StoredAsset = self:FindAsset(UUID);
	local Asset = StoredAsset[1];
	local AssetType = StoredAsset[2];

	if not Asset then
		return false;
	end;

	Asset:Destroy();
	CreatedAssets[UUID] = nil;

	return true;
end;

-- Make a meta table for the Cardboard module so it can be accessed like a function, and used as a class as well for the functions. (Cardboard() and Cardboard:FindAsset())
setmetatable(Cardboard, {
	__call = function(self, ...)
		return self.new(...);
	end;
	__index = Cardboard;
	__newindex = function(self, Index:string, Value)
		error("Attempt to modify read-only property '"..Index.."'");
	end;
});

return Cardboard;