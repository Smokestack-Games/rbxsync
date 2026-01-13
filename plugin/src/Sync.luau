--!strict
--[[
    Sync Module

    Applies changes from the file system back to Studio.
    Handles creating, updating, and deleting instances.
]]

local HttpService = game:GetService("HttpService")
local CollectionService = game:GetService("CollectionService")

local Reflection = require(script.Parent.Reflection)

local Sync = {}

-- Attribute name used to track source file path
local SOURCE_ATTR = "_rbxsync_source"

-- Cache of referenceId -> Instance for resolving references
local instanceCache: {[string]: Instance} = {}

-- Pending reference properties to resolve after all instances are created
-- Format: { { instance: Instance, propName: string, refId: string } }
local pendingReferences: {{instance: Instance, propName: string, refId: string}} = {}

-- Reference properties that need deferred resolution
-- Comprehensive list of all Instance reference properties in Roblox
local refProps = {
    -- Welds and basic constraints (WeldConstraint, Weld, Motor6D, Motor, etc.)
    Part0 = true,
    Part1 = true,
    -- Attachment-based constraints (RopeConstraint, SpringConstraint, HingeConstraint,
    -- BallSocketConstraint, AlignPosition, AlignOrientation, VectorForce, Torque,
    -- LinearVelocity, AngularVelocity, LineForce, PlaneConstraint, PathfindingLink, etc.)
    Attachment0 = true,
    Attachment1 = true,
    -- Model/Assembly
    PrimaryPart = true,
    -- GUI adornments (BillboardGui, SurfaceGui, SelectionBox, Highlight, HandleAdornments)
    Adornee = true,
    -- Value objects (ObjectValue)
    Value = true,
    -- Humanoid references
    HumanoidRootPart = true,
    RootPart = true,
    SeatPart = true,
    WalkToPart = true,
    -- Camera
    CameraSubject = true,
    -- Sound
    SoundGroup = true,
    -- Animation
    Controller = true,
    -- Script linking (deprecated but may exist)
    LinkedSource = true,
    -- BindableEvent/Function targets (if applicable)
    -- These don't typically have instance refs but just in case
}

-- Decode a property value from our JSON format back to Roblox type
local function decodeValue(encoded: any): any
    if encoded == nil then
        return nil
    end

    local valueType = encoded.type
    local value = encoded.value

    if value == nil then
        return nil
    end

    -- Handle primitive types
    if valueType == "bool" then
        return value
    elseif valueType == "int" or valueType == "int64" or valueType == "number" then
        -- Ensure we return a number
        if type(value) == "string" then
            return tonumber(value) or 0
        end
        return value
    elseif valueType == "float" or valueType == "double" then
        if value == "NaN" then
            return 0/0
        elseif value == "Infinity" then
            return math.huge
        elseif value == "-Infinity" then
            return -math.huge
        end
        -- Ensure we return a number
        if type(value) == "string" then
            return tonumber(value) or 0
        end
        return value
    elseif valueType == "string" or valueType == "Content" or valueType == "ProtectedString" then
        -- Check if this was originally a number type
        local originalType = encoded.originalType
        if originalType and (originalType == "int" or originalType == "float" or originalType == "double" or originalType == "number") then
            return tonumber(value) or value
        end
        return value
    end

    -- Handle Vector types
    if valueType == "Vector2" then
        return Vector2.new(value.x, value.y)
    elseif valueType == "Vector2int16" then
        return Vector2int16.new(value.x, value.y)
    elseif valueType == "Vector3" then
        return Vector3.new(value.x, value.y, value.z)
    elseif valueType == "Vector3int16" then
        return Vector3int16.new(value.x, value.y, value.z)
    end

    -- Handle CFrame
    if valueType == "CFrame" then
        local pos = value.position
        local rot = value.rotation
        return CFrame.new(
            pos[1], pos[2], pos[3],
            rot[1], rot[2], rot[3],
            rot[4], rot[5], rot[6],
            rot[7], rot[8], rot[9]
        )
    end

    -- Handle Color types
    if valueType == "Color3" then
        return Color3.new(value.r, value.g, value.b)
    elseif valueType == "Color3uint8" then
        return Color3.fromRGB(value.r, value.g, value.b)
    elseif valueType == "BrickColor" then
        return BrickColor.new(value)
    end

    -- Handle UDim types
    if valueType == "UDim" then
        return UDim.new(value.scale, value.offset)
    elseif valueType == "UDim2" then
        return UDim2.new(
            value.x.scale, value.x.offset,
            value.y.scale, value.y.offset
        )
    end

    -- Handle Rect
    if valueType == "Rect" then
        return Rect.new(value.min.x, value.min.y, value.max.x, value.max.y)
    end

    -- Handle NumberSequence
    if valueType == "NumberSequence" then
        local keypoints = {}
        for _, kp in value.keypoints do
            table.insert(keypoints, NumberSequenceKeypoint.new(kp.time, kp.value, kp.envelope))
        end
        return NumberSequence.new(keypoints)
    end

    -- Handle ColorSequence
    if valueType == "ColorSequence" then
        local keypoints = {}
        for _, kp in value.keypoints do
            table.insert(keypoints, ColorSequenceKeypoint.new(kp.time, Color3.new(kp.color.r, kp.color.g, kp.color.b)))
        end
        return ColorSequence.new(keypoints)
    end

    -- Handle NumberRange
    if valueType == "NumberRange" then
        return NumberRange.new(value.min, value.max)
    end

    -- Handle Enum
    if valueType == "Enum" then
        local enumType = value.enumType
        local enumValue = value.value
        local ok, result = pcall(function()
            return (Enum :: any)[enumType][enumValue]
        end)
        if ok then
            return result
        end
        return nil
    end

    -- Handle Instance references
    if valueType == "Ref" then
        if value and instanceCache[value] then
            return instanceCache[value]
        end
        return nil
    end

    -- Handle Font
    if valueType == "Font" then
        local weight = Enum.FontWeight[value.weight] or Enum.FontWeight.Regular
        local style = Enum.FontStyle[value.style] or Enum.FontStyle.Normal
        return Font.new(value.family, weight, style)
    end

    -- Handle Faces
    if valueType == "Faces" then
        return Faces.new(
            value.top and Enum.NormalId.Top or nil,
            value.bottom and Enum.NormalId.Bottom or nil,
            value.left and Enum.NormalId.Left or nil,
            value.right and Enum.NormalId.Right or nil,
            value.front and Enum.NormalId.Front or nil,
            value.back and Enum.NormalId.Back or nil
        )
    end

    -- Handle Axes
    if valueType == "Axes" then
        local axes = {}
        if value.x then table.insert(axes, Enum.Axis.X) end
        if value.y then table.insert(axes, Enum.Axis.Y) end
        if value.z then table.insert(axes, Enum.Axis.Z) end
        return Axes.new(unpack(axes))
    end

    -- Handle PhysicalProperties
    if valueType == "PhysicalProperties" then
        if value then
            return PhysicalProperties.new(
                value.density,
                value.friction,
                value.elasticity,
                value.frictionWeight,
                value.elasticityWeight
            )
        end
        return nil
    end

    -- Handle Ray
    if valueType == "Ray" then
        return Ray.new(
            Vector3.new(value.origin.x, value.origin.y, value.origin.z),
            Vector3.new(value.direction.x, value.direction.y, value.direction.z)
        )
    end

    -- Handle Region3
    if valueType == "Region3" then
        local min = Vector3.new(value.min.x, value.min.y, value.min.z)
        local max = Vector3.new(value.max.x, value.max.y, value.max.z)
        return Region3.new(min, max)
    end

    -- Handle Region3int16
    if valueType == "Region3int16" then
        local min = Vector3int16.new(value.min.x, value.min.y, value.min.z)
        local max = Vector3int16.new(value.max.x, value.max.y, value.max.z)
        return Region3int16.new(min, max)
    end

    -- Handle OptionalCFrame (OptionalCoordinateFrame)
    if valueType == "OptionalCFrame" or valueType == "OptionalCoordinateFrame" then
        if value then
            local pos = value.position
            local rot = value.rotation
            return CFrame.new(
                pos[1], pos[2], pos[3],
                rot[1], rot[2], rot[3],
                rot[4], rot[5], rot[6],
                rot[7], rot[8], rot[9]
            )
        end
        return nil
    end

    -- Fallback - return nil for unknown types
    return nil
end

-- Find or create instance at path
-- Also tracks recently created instances that might have different names
local recentlyCreated: {[string]: Instance} = {}

-- Track instances by referenceId for reliable lookup
local instanceByRefId: {[string]: Instance} = {}

-- Unescape path segment back to original name
local function unescapePathSegment(segment: string): string
    return string.gsub(segment, "%[SLASH%]", "/")
end

-- Strip disambiguation suffix from a path segment
-- e.g., "TrimWall~2~" -> "TrimWall", "Part~3~" -> "Part"
local function stripDisambiguationSuffix(segment: string): (string, number?)
    -- Match pattern: name ending with ~N~ where N is one or more digits
    local baseName, indexStr = string.match(segment, "^(.+)~(%d+)~$")
    if baseName then
        return baseName, tonumber(indexStr)
    end
    return segment, nil
end

-- Find a child instance by name (with escaping/disambiguation handling)
local function findChildByName(parent: Instance, pathSegment: string): Instance?
    -- First unescape to get the search name
    local baseName, duplicateIndex = stripDisambiguationSuffix(pathSegment)
    local searchName = unescapePathSegment(baseName)

    -- If it's a disambiguated name (has ~N~ suffix), find the Nth instance with that name
    if duplicateIndex then
        local count = 0
        for _, child in parent:GetChildren() do
            if child.Name == searchName then
                count = count + 1
                if count == duplicateIndex then
                    return child
                end
            end
        end
        -- If we didn't find the Nth instance, return nil (don't fallback to wrong instance)
        -- This allows syncCreate to make a new instance
        return nil
    else
        -- No disambiguation - try exact match first (with unescaping)
        local child = parent:FindFirstChild(searchName)
        if child then
            return child
        end

        -- Try finding by source attribute
        for _, candidate in parent:GetChildren() do
            local sourceAttr = candidate:GetAttribute(SOURCE_ATTR)
            if sourceAttr and string.find(sourceAttr, pathSegment, 1, true) then
                return candidate
            end
        end
    end

    return nil
end

-- Register an instance by its referenceId for lookup
function Sync.registerInstanceByRefId(refId: string, instance: Instance)
    if refId and refId ~= "" then
        instanceByRefId[refId] = instance
    end
end

-- Find instance by referenceId
function Sync.findInstanceByRefId(refId: string): Instance?
    if not refId or refId == "" then
        return nil
    end

    -- Check our cache first
    local cached = instanceByRefId[refId]
    if cached and cached.Parent then
        return cached
    end

    -- Search by attribute in game tree
    local services = {
        "Workspace", "ReplicatedStorage", "ReplicatedFirst",
        "ServerScriptService", "ServerStorage", "StarterGui",
        "StarterPack", "StarterPlayer", "Lighting", "SoundService",
        "Teams", "Chat", "LocalizationService", "TestService"
    }

    for _, serviceName in services do
        local service = game:FindFirstChild(serviceName)
        if service then
            for _, desc in service:GetDescendants() do
                local attr = desc:GetAttribute("_rbxsync_ref")
                if attr == refId then
                    instanceByRefId[refId] = desc
                    return desc
                end
            end
        end
    end

    return nil
end

function Sync.findInstanceAtPath(path: string): Instance?
    -- Normalize path delimiters (Windows uses backslash, we use forward slash)
    path = string.gsub(path, "\\", "/")

    -- First check if we created this recently (handles name mismatches)
    if recentlyCreated[path] then
        local inst = recentlyCreated[path]
        if inst.Parent then  -- Still valid
            return inst
        end
    end

    local parts = string.split(path, "/")

    -- Top-level service case
    if #parts == 1 then
        return game:FindFirstChild(unescapePathSegment(parts[1]))
    end

    -- Walk the tree to find the instance
    local current: Instance = game
    for i, part in parts do
        local child = findChildByName(current, part)
        if not child then
            -- Try recently created
            local partialPath = table.concat(parts, "/", 1, i)
            if recentlyCreated[partialPath] then
                child = recentlyCreated[partialPath]
            end
        end
        if not child then
            return nil
        end
        current = child
    end

    return current
end

-- Register a created instance for path lookup
function Sync.registerCreated(path: string, instance: Instance)
    -- Normalize path delimiters
    path = string.gsub(path, "\\", "/")
    recentlyCreated[path] = instance
end

-- Clear the recently created cache
function Sync.clearRecentlyCreated()
    table.clear(recentlyCreated)
end

-- Create an instance from serialized data
function Sync.createInstance(data: any, parent: Instance?): Instance?
    local className = data.className
    local referenceId = data.referenceId

    -- Always use data.name (the original instance name from JSON)
    -- The path may have escaping/disambiguation that shouldn't affect the actual name
    local name = data.name
    if not name then
        name = className or "Instance"
    end

    -- Don't create services, just find them
    local services = {
        "Workspace", "ReplicatedStorage", "ReplicatedFirst",
        "ServerScriptService", "ServerStorage", "StarterGui",
        "StarterPack", "StarterPlayer", "Lighting", "SoundService",
        "Teams", "Chat", "LocalizationService", "TestService"
    }

    for _, serviceName in services do
        if className == serviceName or name == serviceName then
            local service = game:FindFirstChild(serviceName)
            if service and referenceId then
                instanceCache[referenceId] = service
                Sync.registerInstanceByRefId(referenceId, service)
            end
            return service
        end
    end

    -- Special handling for MeshPart - use CreateMeshPartAsync to set MeshId
    -- (MeshId property is not writable by plugins, must be set at creation)
    if className == "MeshPart" and data.properties and data.properties.MeshId then
        local meshIdData = data.properties.MeshId
        local meshId = meshIdData.value
        if meshId and meshId ~= "" then
            local AssetService = game:GetService("AssetService")
            local ok, instance = pcall(function()
                return AssetService:CreateMeshPartAsync(meshId)
            end)
            if ok and instance then
                instance.Name = name
                -- Cache for reference resolution
                if referenceId then
                    instanceCache[referenceId] = instance
                    Sync.registerInstanceByRefId(referenceId, instance)
                end
                -- Store referenceId as attribute
                if referenceId then
                    pcall(function()
                        instance:SetAttribute("_rbxsync_ref", referenceId)
                    end)
                end
                -- Apply other properties (excluding MeshId which is already set)
                if data.properties then
                    local propsWithoutMeshId = {}
                    for propName, propValue in data.properties do
                        if propName ~= "MeshId" then
                            propsWithoutMeshId[propName] = propValue
                        end
                    end
                    Sync.applyProperties(instance, propsWithoutMeshId)
                end
                -- Handle attributes, tags, etc.
                if data.attributes then
                    for attrName, attrData in data.attributes do
                        if string.sub(attrName, 1, 4) == "RBX_" then
                            continue
                        end
                        local attrValue = decodeValue(attrData)
                        if attrValue ~= nil then
                            pcall(function()
                                instance:SetAttribute(attrName, attrValue)
                            end)
                        end
                    end
                end
                if data.tags then
                    for _, tag in data.tags do
                        CollectionService:AddTag(instance, tag)
                    end
                end
                if data.path then
                    pcall(function()
                        instance:SetAttribute(SOURCE_ATTR, data.path)
                    end)
                end
                if parent then
                    instance.Parent = parent
                end
                return instance
            end
            -- If CreateMeshPartAsync failed, fall through to normal creation
        end
    end

    -- Create new instance
    local ok, instance = pcall(function()
        return Instance.new(className)
    end)

    if not ok or not instance then
        return nil
    end

    -- Set name first (use the original name, not escaped/disambiguated)
    instance.Name = name

    -- Cache for reference resolution
    if referenceId then
        instanceCache[referenceId] = instance
        Sync.registerInstanceByRefId(referenceId, instance)
    end

    -- Store referenceId as attribute for future lookups
    if referenceId then
        pcall(function()
            instance:SetAttribute("_rbxsync_ref", referenceId)
        end)
    end

    -- Apply properties
    if data.properties then
        Sync.applyProperties(instance, data.properties)
    end

    -- Handle script source
    if data.source and (className == "Script" or className == "LocalScript" or className == "ModuleScript") then
        pcall(function()
            (instance :: any).Source = data.source
        end)
    end

    -- Apply attributes (skip reserved RBX_ attributes)
    if data.attributes then
        for attrName, attrData in data.attributes do
            -- Skip Roblox internal attributes
            if string.sub(attrName, 1, 4) == "RBX_" then
                continue
            end
            local attrValue = decodeValue(attrData)
            if attrValue ~= nil then
                pcall(function()
                    instance:SetAttribute(attrName, attrValue)
                end)
            end
        end
    end

    -- Apply tags
    if data.tags then
        for _, tag in data.tags do
            CollectionService:AddTag(instance, tag)
        end
    end

    -- Set source tracking attribute (links instance to file path)
    if data.path then
        pcall(function()
            instance:SetAttribute(SOURCE_ATTR, data.path)
        end)
    end

    -- Set parent last (triggers instance creation in workspace)
    if parent then
        instance.Parent = parent
    end

    return instance
end

-- Apply properties to an existing instance
function Sync.applyProperties(instance: Instance, properties: {[string]: any}): ()
    -- Properties that should be completely skipped (read-only or problematic)
    local skipProps = {
        Name = true,
        Parent = true,
        ClassName = true,
        -- Content properties (don't serialize correctly)
        Capabilities = true,
        TextureContent = true,
        EmissiveMaskContent = true,
        ColorMapContent = true,
        NormalMapContent = true,
        MetalnessMapContent = true,
        RoughnessMapContent = true,
        AudioContent = true,
        ImageContent = true,
        MoonTextureContent = true,
        SunTextureContent = true,
        SkyboxUpContent = true,
        SkyboxDownContent = true,
        SkyboxLeftContent = true,
        SkyboxRightContent = true,
        SkyboxFrontContent = true,
        SkyboxBackContent = true,
        -- Other problematic properties
        CurrentCamera = true,
        TargetInstance = true,
        SourceInstance = true,
        UniqueId = true,
        HistoryId = true,
        SourceAssetId = true,
        ScriptGuid = true,
    }

    for propName, propData in properties do
        if skipProps[propName] then
            continue
        end

        -- For reference properties, queue them for later resolution
        if refProps[propName] and propData and propData.type == "Ref" then
            local refId = propData.value
            if refId and refId ~= "" then
                table.insert(pendingReferences, {
                    instance = instance,
                    propName = propName,
                    refId = refId,
                })
            end
            continue
        end

        local value = decodeValue(propData)

        -- Try to set the property
        local ok, err = pcall(function()
            (instance :: any)[propName] = value
        end)

        if not ok then
            -- Property might be read-only or not exist, that's okay
            -- warn("[RbxSync] Could not set", instance.ClassName .. "." .. propName, err)
        end
    end
end

-- Update an existing instance with new data
-- Returns: boolean success, Instance? replacedInstance (if MeshPart was recreated)
function Sync.updateInstance(instance: Instance, data: any): (boolean, Instance?)
    -- Special handling for MeshPart: MeshId is read-only after creation
    -- If MeshId differs, we need to recreate the MeshPart entirely
    if instance:IsA("MeshPart") and data.properties and data.properties.MeshId then
        local newMeshId = data.properties.MeshId.value
        local currentMeshId = (instance :: MeshPart).MeshId

        -- Check if MeshId needs to change
        if newMeshId and newMeshId ~= "" and newMeshId ~= currentMeshId then
            local AssetService = game:GetService("AssetService")
            local ok, newMeshPart = pcall(function()
                return AssetService:CreateMeshPartAsync(newMeshId)
            end)

            if ok and newMeshPart then
                -- Copy name
                newMeshPart.Name = data.name or instance.Name

                -- Apply properties (excluding MeshId which is already set)
                if data.properties then
                    local propsWithoutMeshId = {}
                    for propName, propValue in data.properties do
                        if propName ~= "MeshId" then
                            propsWithoutMeshId[propName] = propValue
                        end
                    end
                    Sync.applyProperties(newMeshPart, propsWithoutMeshId)
                end

                -- Copy attributes
                for attrName, attrValue in instance:GetAttributes() do
                    pcall(function()
                        newMeshPart:SetAttribute(attrName, attrValue)
                    end)
                end

                -- Apply new attributes from data
                if data.attributes then
                    for attrName, attrData in data.attributes do
                        if string.sub(attrName, 1, 4) ~= "RBX_" then
                            local attrValue = decodeValue(attrData)
                            if attrValue ~= nil then
                                pcall(function()
                                    newMeshPart:SetAttribute(attrName, attrValue)
                                end)
                            end
                        end
                    end
                end

                -- Copy tags
                for _, tag in CollectionService:GetTags(instance) do
                    CollectionService:AddTag(newMeshPart, tag)
                end

                -- Apply new tags from data
                if data.tags then
                    for _, tag in data.tags do
                        CollectionService:AddTag(newMeshPart, tag)
                    end
                end

                -- Move children to new MeshPart
                for _, child in instance:GetChildren() do
                    child.Parent = newMeshPart
                end

                -- Set source tracking
                if data.path then
                    pcall(function()
                        newMeshPart:SetAttribute(SOURCE_ATTR, data.path)
                    end)
                end

                -- Cache for reference resolution
                if data.referenceId then
                    instanceCache[data.referenceId] = newMeshPart
                    Sync.registerInstanceByRefId(data.referenceId, newMeshPart)
                end

                -- Replace in parent
                local parent = instance.Parent
                newMeshPart.Parent = parent
                instance:Destroy()

                return true, newMeshPart
            end
            -- If CreateMeshPartAsync failed, fall through to normal update
        end
    end

    -- Ensure source tracking attribute is set (for older instances)
    if data.path then
        pcall(function()
            instance:SetAttribute(SOURCE_ATTR, data.path)
        end)
    end

    -- Update name if provided (use JSON name, not file name)
    if data.name and instance.Name ~= data.name then
        instance.Name = data.name
    end

    -- Update properties
    if data.properties then
        Sync.applyProperties(instance, data.properties)
    end

    -- Update script source
    if data.source and (instance:IsA("LuaSourceContainer")) then
        pcall(function()
            (instance :: any).Source = data.source
        end)
    end

    -- Update attributes (clear and re-apply, skip reserved RBX_ attributes and source tracking)
    if data.attributes then
        -- Remove existing non-reserved attributes
        for name, _ in instance:GetAttributes() do
            if string.sub(name, 1, 4) ~= "RBX_" and name ~= SOURCE_ATTR then
                instance:SetAttribute(name, nil)
            end
        end
        -- Apply new attributes
        for attrName, attrData in data.attributes do
            -- Skip Roblox internal attributes
            if string.sub(attrName, 1, 4) == "RBX_" then
                continue
            end
            local attrValue = decodeValue(attrData)
            if attrValue ~= nil then
                pcall(function()
                    instance:SetAttribute(attrName, attrValue)
                end)
            end
        end
    end

    -- Update tags (clear and re-apply)
    if data.tags then
        -- Remove existing tags
        for _, tag in CollectionService:GetTags(instance) do
            CollectionService:RemoveTag(instance, tag)
        end
        -- Apply new tags
        for _, tag in data.tags do
            CollectionService:AddTag(instance, tag)
        end
    end

    -- Cache for reference resolution
    if data.referenceId then
        instanceCache[data.referenceId] = instance
    end

    return true, nil
end

-- Delete an instance by path
-- Uses Parent = nil instead of Destroy() to allow undo
function Sync.deleteInstance(path: string): boolean
    -- Normalize path delimiters
    path = string.gsub(path, "\\", "/")
    local instance = Sync.findInstanceAtPath(path)
    if instance then
        instance.Parent = nil
        return true
    end
    return false
end

-- Compare two encoded values for equality
local function valuesEqual(encoded1: any, encoded2: any): boolean
    if encoded1 == nil and encoded2 == nil then
        return true
    end
    if encoded1 == nil or encoded2 == nil then
        return false
    end
    if type(encoded1) ~= "table" or type(encoded2) ~= "table" then
        return encoded1 == encoded2
    end

    -- Compare type
    if encoded1.type ~= encoded2.type then
        return false
    end

    local v1, v2 = encoded1.value, encoded2.value

    -- Handle nil values
    if v1 == nil and v2 == nil then
        return true
    end
    if v1 == nil or v2 == nil then
        return false
    end

    -- Primitives
    if type(v1) ~= "table" then
        return v1 == v2
    end

    -- Deep compare tables
    for k, val1 in v1 do
        local val2 = v2[k]
        if type(val1) == "table" and type(val2) == "table" then
            -- Recurse for nested tables
            for kk, vv1 in val1 do
                if val2[kk] ~= vv1 then
                    return false
                end
            end
        elseif val1 ~= val2 then
            return false
        end
    end

    return true
end

-- Check if an instance needs updating based on incoming data
-- Returns: needsUpdate (boolean), reasons (table of strings or nil)
function Sync.instanceNeedsUpdate(instance: Instance, data: any): (boolean, {string}?)
    local reasons: {string} = {}
    local DEBUG = false -- Enable verbose debugging

    -- Check script source (most common change)
    if instance:IsA("LuaSourceContainer") then
        local currentSource = ""
        pcall(function()
            currentSource = (instance :: any).Source
        end)

        -- Get incoming source from data.properties.Source or data.source
        local newSource = data.source
        if not newSource and data.properties and data.properties.Source then
            newSource = data.properties.Source.value
        end

        if newSource and currentSource ~= newSource then
            table.insert(reasons, "source")
            if DEBUG then
                print(string.format("[DEBUG] %s.Source differs: len=%d vs len=%d", instance:GetFullName(), #currentSource, #newSource))
            end
        end
    end

    -- Check name
    if data.name and instance.Name ~= data.name then
        table.insert(reasons, "name:" .. instance.Name .. "->" .. data.name)
        if DEBUG then
            print(string.format("[DEBUG] %s name differs: '%s' vs '%s'", instance:GetFullName(), instance.Name, data.name))
        end
    end

    -- Check properties (excluding Source which we already checked)
    -- Skip properties that can't be properly compared or are read-only
    local skipProps = {
        Name = true,
        Parent = true,
        ClassName = true,
        Source = true,
        -- Content properties (don't compare correctly)
        Capabilities = true,
        TextureContent = true,
        EmissiveMaskContent = true,
        ColorMapContent = true,
        NormalMapContent = true,
        MetalnessMapContent = true,
        RoughnessMapContent = true,
        AudioContent = true,
        ImageContent = true,
        MoonTextureContent = true,
        SunTextureContent = true,
        SkyboxUpContent = true,
        SkyboxDownContent = true,
        SkyboxLeftContent = true,
        SkyboxRightContent = true,
        SkyboxFrontContent = true,
        SkyboxBackContent = true,
        -- Instance reference properties (can't compare across sessions)
        PrimaryPart = true,
        CurrentCamera = true,
        Part0 = true,
        Part1 = true,
        Attachment0 = true,
        Attachment1 = true,
        TargetInstance = true,
        SourceInstance = true,
        -- Other problematic properties
        UniqueId = true,
        HistoryId = true,
        SourceAssetId = true,
        ScriptGuid = true,
    }

    if data.properties then
        for propName, propData in data.properties do
            if skipProps[propName] then
                continue
            end

            -- Get current value
            local ok, currentValue = pcall(function()
                return (instance :: any)[propName]
            end)

            if not ok then
                continue  -- Can't read property, skip
            end

            -- Decode incoming value
            local newValue = decodeValue(propData)

            -- Compare values (simple comparison for now)
            if currentValue ~= newValue then
                -- For complex types, we need deeper comparison
                if type(currentValue) == "userdata" or type(newValue) == "userdata" then
                    -- Convert to string for comparison
                    local currentStr = tostring(currentValue)
                    local newStr = tostring(newValue)
                    if currentStr ~= newStr then
                        table.insert(reasons, propName)
                        if DEBUG then
                            print(string.format("[DEBUG] %s.%s differs (userdata): '%s' vs '%s'", instance:GetFullName(), propName, currentStr, newStr))
                        end
                    end
                else
                    table.insert(reasons, propName)
                    if DEBUG then
                        print(string.format("[DEBUG] %s.%s differs: type=%s/%s, current='%s', new='%s'",
                            instance:GetFullName(), propName,
                            type(currentValue), type(newValue),
                            tostring(currentValue), tostring(newValue)))
                    end
                end
            end
        end
    end

    -- Check attributes
    if data.attributes then
        local currentAttrs = instance:GetAttributes()

        -- Check for new or changed attributes
        for attrName, attrData in data.attributes do
            if string.sub(attrName, 1, 4) == "RBX_" then
                continue
            end
            local newValue = decodeValue(attrData)
            if currentAttrs[attrName] ~= newValue then
                table.insert(reasons, "attr:" .. attrName)
                if DEBUG then
                    print(string.format("[DEBUG] %s attr '%s' differs: '%s' vs '%s'",
                        instance:GetFullName(), attrName,
                        tostring(currentAttrs[attrName]), tostring(newValue)))
                end
            end
        end

        -- Check for removed attributes (existing attrs not in new data)
        for attrName, _ in currentAttrs do
            if string.sub(attrName, 1, 4) == "RBX_" then
                continue
            end
            if attrName == SOURCE_ATTR or attrName == "_rbxsync_ref" then
                continue -- Skip our internal attributes
            end
            if data.attributes[attrName] == nil then
                table.insert(reasons, "-attr:" .. attrName)
                if DEBUG then
                    print(string.format("[DEBUG] %s has extra attr '%s' not in data", instance:GetFullName(), attrName))
                end
            end
        end
    end

    -- Check tags
    if data.tags then
        local currentTags = CollectionService:GetTags(instance)

        -- Quick length check
        if #currentTags ~= #data.tags then
            table.insert(reasons, "tags")
            if DEBUG then
                print(string.format("[DEBUG] %s tags count differs: %d vs %d", instance:GetFullName(), #currentTags, #data.tags))
            end
        else
            -- Check each tag
            local tagSet = {}
            for _, tag in currentTags do
                tagSet[tag] = true
            end
            for _, tag in data.tags do
                if not tagSet[tag] then
                    table.insert(reasons, "tags")
                    if DEBUG then
                        print(string.format("[DEBUG] %s missing tag: '%s'", instance:GetFullName(), tag))
                    end
                    break
                end
            end
        end
    end

    if #reasons > 0 then
        return true, reasons
    end
    return false, nil
end

-- Clear all caches
function Sync.clearCache(): ()
    table.clear(instanceCache)
    table.clear(instanceByRefId)
    table.clear(recentlyCreated)
    table.clear(pendingReferences)
end

-- Clear pending references (call before starting a sync batch)
function Sync.clearPendingReferences(): ()
    table.clear(pendingReferences)
end

-- Resolve all pending reference properties
-- Call this AFTER all instances have been created
function Sync.resolvePendingReferences(): number
    local resolved = 0
    for _, pending in pendingReferences do
        local instance = pending.instance
        local propName = pending.propName
        local refId = pending.refId

        -- Skip if instance was destroyed
        if not instance.Parent then
            continue
        end

        -- Try to find the referenced instance
        local target = instanceCache[refId] or instanceByRefId[refId]
        if target and target.Parent then
            local ok = pcall(function()
                (instance :: any)[propName] = target
            end)
            if ok then
                resolved += 1
            end
        end
    end

    table.clear(pendingReferences)
    return resolved
end

-- Build instance cache from current game state
function Sync.buildCacheFromGame(): ()
    Sync.clearCache()

    local services = {
        "Workspace", "ReplicatedStorage", "ReplicatedFirst",
        "ServerScriptService", "ServerStorage", "StarterGui",
        "StarterPack", "StarterPlayer", "Lighting", "SoundService",
        "Teams", "Chat", "LocalizationService", "TestService"
    }

    for _, serviceName in services do
        local service = game:FindFirstChild(serviceName)
        if service then
            for _, descendant in service:GetDescendants() do
                -- Use full name as cache key for now
                local fullName = descendant:GetFullName()
                instanceCache[fullName] = descendant
            end
        end
    end
end

return Sync
