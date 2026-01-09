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
    elseif valueType == "int" or valueType == "int64" then
        return value
    elseif valueType == "float" or valueType == "double" then
        if value == "NaN" then
            return 0/0
        elseif value == "Infinity" then
            return math.huge
        elseif value == "-Infinity" then
            return -math.huge
        end
        return value
    elseif valueType == "string" or valueType == "Content" or valueType == "ProtectedString" then
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

function Sync.findInstanceAtPath(path: string): Instance?
    -- First check if we created this recently (handles name mismatches)
    if recentlyCreated[path] then
        local inst = recentlyCreated[path]
        if inst.Parent then  -- Still valid
            return inst
        end
    end

    -- Search for instance by source attribute (allows name to differ from path)
    -- Get the parent path to narrow the search
    local parts = string.split(path, "/")
    local parentPath = table.concat(parts, "/", 1, #parts - 1)
    local parent: Instance? = nil

    if #parts > 1 then
        -- Find parent by walking tree
        local current: Instance = game
        for i = 1, #parts - 1 do
            local child = current:FindFirstChild(parts[i])
            if not child then
                break
            end
            current = child
        end
        parent = current
    else
        -- Top-level service
        parent = game:FindFirstChild(parts[1])
        if parent then
            return parent  -- Services don't need attribute tracking
        end
    end

    -- Search children of parent for matching source attribute
    if parent then
        for _, child in parent:GetChildren() do
            local sourceAttr = child:GetAttribute(SOURCE_ATTR)
            if sourceAttr == path then
                return child
            end
        end
    end

    -- Fallback: try traditional path-based lookup
    local current: Instance = game
    for i, part in parts do
        local child = current:FindFirstChild(part)
        if not child then
            -- Try to find by checking recently created with partial path
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

    -- Derive name from path if not provided
    local name = data.name
    if not name and data.path then
        -- Extract last segment of path as name
        local parts = string.split(data.path, "/")
        name = parts[#parts]
    end
    name = name or className or "Instance"

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
            end
            return service
        end
    end

    -- Create new instance
    local ok, instance = pcall(function()
        return Instance.new(className)
    end)

    if not ok or not instance then
        return nil
    end

    -- Set name first
    instance.Name = name

    -- Cache for reference resolution
    if referenceId then
        instanceCache[referenceId] = instance
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
    -- Properties that should be set in a specific order or skipped
    local skipProps = {
        Name = true,
        Parent = true,
        ClassName = true,
    }

    for propName, propData in properties do
        if skipProps[propName] then
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
function Sync.updateInstance(instance: Instance, data: any): boolean
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

    return true
end

-- Delete an instance by path
-- Uses Parent = nil instead of Destroy() to allow undo
function Sync.deleteInstance(path: string): boolean
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
-- Returns: needsUpdate (boolean), reason (string or nil)
function Sync.instanceNeedsUpdate(instance: Instance, data: any): (boolean, string?)
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
            return true, "source"
        end
    end

    -- Check name
    if data.name and instance.Name ~= data.name then
        return true, "name"
    end

    -- Check properties (excluding Source which we already checked)
    if data.properties then
        for propName, propData in data.properties do
            if propName == "Name" or propName == "Parent" or propName == "ClassName" or propName == "Source" then
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
                        return true, "property:" .. propName
                    end
                else
                    return true, "property:" .. propName
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
                return true, "attribute:" .. attrName
            end
        end

        -- Check for removed attributes (existing attrs not in new data)
        for attrName, _ in currentAttrs do
            if string.sub(attrName, 1, 4) == "RBX_" then
                continue
            end
            if data.attributes[attrName] == nil then
                return true, "attribute_removed:" .. attrName
            end
        end
    end

    -- Check tags
    if data.tags then
        local currentTags = CollectionService:GetTags(instance)

        -- Quick length check
        if #currentTags ~= #data.tags then
            return true, "tags"
        end

        -- Check each tag
        local tagSet = {}
        for _, tag in currentTags do
            tagSet[tag] = true
        end
        for _, tag in data.tags do
            if not tagSet[tag] then
                return true, "tags"
            end
        end
    end

    return false, nil
end

-- Clear the instance cache
function Sync.clearCache(): ()
    table.clear(instanceCache)
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
