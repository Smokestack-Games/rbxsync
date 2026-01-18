--!strict
--[[
    Sync Module

    Applies changes from the file system back to Studio.
    Handles creating, updating, and deleting instances.
]]

local HttpService = game:GetService("HttpService")
local CollectionService = game:GetService("CollectionService")

local Reflection = require(script.Parent.Reflection)
local CSGHandler = require(script.Parent.CSGHandler)
local ChangeTracker = require(script.Parent.ChangeTracker)

local Sync = {}

-- Attribute name used to track source file path
local SOURCE_ATTR = "_rbxsync_source"

-- Cache of referenceId -> Instance for resolving references
local instanceCache: {[string]: Instance} = {}

-- Pending reference properties to resolve after all instances are created
-- Format: { { instance: Instance, propName: string, refId: string } }
local pendingReferences: {{instance: Instance, propName: string, refId: string}} = {}

-- Pending material overrides to apply after all MaterialVariants exist
-- Format: { { materialName: string, variantName: string } }
local pendingMaterialOverrides: {{materialName: string, variantName: string}} = {}

-- Pending CSG reconstructions to perform after all parts exist
-- Format: { { path: string, data: any, parent: Instance } }
local pendingCSGReconstructions: {{path: string, data: any, parent: Instance}} = {}

-- Track instances by path for CSG reconstruction
local instancesByPath: {[string]: Instance} = {}

-- Services that can be accessed via game:GetService()
-- Used for reliable service lookup instead of FindFirstChild
local SERVICES: {[string]: boolean} = {
    Workspace = true,
    ReplicatedStorage = true,
    ReplicatedFirst = true,
    ServerScriptService = true,
    ServerStorage = true,
    StarterGui = true,
    StarterPack = true,
    StarterPlayer = true,
    Lighting = true,
    SoundService = true,
    Teams = true,
    Chat = true,
    LocalizationService = true,
    TestService = true,
    MaterialService = true,
}

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
    -- BUT we must match by _rbxsync_source first to avoid counting wrong instances
    if duplicateIndex then
        -- First try to find by exact source match
        for _, child in parent:GetChildren() do
            local sourceAttr = child:GetAttribute(SOURCE_ATTR)
            if sourceAttr then
                local sourceLastSegment = string.match(sourceAttr, "[^/]+$")
                if sourceLastSegment == pathSegment then
                    return child
                end
            end
        end

        -- If no source match, count by name (for fresh syncs)
        -- But be careful not to count instances that already have a different source
        local count = 0
        for _, child in parent:GetChildren() do
            if child.Name == searchName then
                local sourceAttr = child:GetAttribute(SOURCE_ATTR)
                -- Skip if this child already has a source that doesn't match our path
                if sourceAttr then
                    local sourceLastSegment = string.match(sourceAttr, "[^/]+$")
                    if sourceLastSegment and sourceLastSegment ~= pathSegment then
                        -- This belongs to a different path, don't count it
                        continue
                    end
                end
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
        -- No disambiguation - MUST match by _rbxsync_source attribute first
        -- to avoid accidentally matching a Part that was created for a different path
        -- (e.g., Part~10~ being matched when looking for Part)
        for _, candidate in parent:GetChildren() do
            local sourceAttr = candidate:GetAttribute(SOURCE_ATTR)
            if sourceAttr then
                local sourceLastSegment = string.match(sourceAttr, "[^/]+$")
                if sourceLastSegment == pathSegment or sourceLastSegment == searchName then
                    return candidate
                end
            end
        end

        -- Only if no source match, try exact name match
        local child = parent:FindFirstChild(searchName)
        if child then
            -- But verify it doesn't have a different source path!
            local existingSource = child:GetAttribute(SOURCE_ATTR)
            if existingSource then
                local existingLastSegment = string.match(existingSource, "[^/]+$")
                -- If this instance was already assigned a different source path, don't match it
                if existingLastSegment and existingLastSegment ~= pathSegment and existingLastSegment ~= searchName then
                    child = nil
                end
            end
            if child then
                return child
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
-- Note: We only use in-memory cache, not attributes, to avoid conflicts with ScriptSync
function Sync.findInstanceByRefId(refId: string): Instance?
    if not refId or refId == "" then
        return nil
    end

    -- Check our cache (no longer search by attribute to avoid ScriptSync conflicts)
    local cached = instanceByRefId[refId]
    if cached and cached.Parent then
        return cached
    end

    return nil
end

function Sync.findInstanceAtPath(path: string): Instance?
    -- Normalize path delimiters (Windows uses backslash, we use forward slash)
    path = string.gsub(path, "\\", "/")

    -- First check if we created this recently (handles name mismatches)
    if recentlyCreated[path] then
        local inst = recentlyCreated[path]
        if inst.Parent then  -- Still valid (not orphaned)
            return inst
        end
        -- Instance was destroyed or orphaned, don't return stale cache entry
    end

    local parts = string.split(path, "/")

    -- Top-level service case - use GetService for services (more reliable than FindFirstChild)
    if #parts == 1 then
        local name = unescapePathSegment(parts[1])
        if SERVICES[name] then
            local ok, service = pcall(function()
                return game:GetService(name)
            end)
            if ok and service then
                return service
            end
        end
        return game:FindFirstChild(name)
    end

    -- Walk the tree to find the instance
    local current: Instance = game
    for i, part in parts do
        local child = findChildByName(current, part)
        if not child then
            -- Try recently created (but only if still parented)
            local partialPath = table.concat(parts, "/", 1, i)
            local cached = recentlyCreated[partialPath]
            if cached and cached.Parent then
                child = cached
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

    -- Early check for missing className
    if not className then
        warn("[Sync.createInstance] Missing className in data, path:", data.path or "unknown")
        return nil
    end

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
        "Teams", "Chat", "LocalizationService", "TestService",
        "MaterialService"
    }

    for _, serviceName in services do
        if className == serviceName or name == serviceName then
            local service = game:GetService(serviceName)
            if service and referenceId then
                instanceCache[referenceId] = service
                Sync.registerInstanceByRefId(referenceId, service)
            end
            -- Queue material overrides for MaterialService (apply after all MaterialVariants exist)
            if serviceName == "MaterialService" and data.materialOverrides then
                for materialName, variantName in data.materialOverrides do
                    table.insert(pendingMaterialOverrides, {
                        materialName = materialName,
                        variantName = variantName,
                    })
                end
                print(string.format("[RbxSync] Queued %d material overrides for later application", #pendingMaterialOverrides))
            end
            return service
        end
    end

    -- Special handling for UnionOperation/IntersectOperation with CSG data
    -- If separable, queue for reconstruction instead of creating empty shell
    if (className == "UnionOperation" or className == "IntersectOperation") and data.csg then
        if data.csg.separable and data.csg.operations and #data.csg.operations > 0 then
            -- Queue for reconstruction after all parts are created
            table.insert(pendingCSGReconstructions, {
                path = data.path,
                data = data,
                parent = parent :: Instance,
            })
            print(string.format("[RbxSync] Queued union '%s' for CSG reconstruction (%d parts)", name, #data.csg.operations))
            return nil  -- Don't create the instance yet
        elseif data.csg.error then
            warn(string.format("[RbxSync] Union '%s' not separable: %s", name, data.csg.error))
            -- Fall through to create empty shell
        end
    end

    -- Special handling for CSG component parts (paths containing /_csg/)
    -- These are temporary parts used for union reconstruction
    -- Create them without a parent, track by path for later lookup
    local isCSGPart = data.path and string.find(data.path, "/_csg/") ~= nil
    if isCSGPart then
        -- For CSG operations (NegateOperation, UnionOperation, IntersectOperation),
        -- create a regular Part instead. These via Instance.new() create empty shells
        -- with no geometry. SubtractAsync/UnionAsync need actual geometry.
        local createClassName = className
        if className == "NegateOperation"
            or className == "UnionOperation"
            or className == "IntersectOperation" then
            createClassName = "Part"
        end

        -- Create the part without parenting (will be used for union reconstruction)
        local ok, instance = pcall(function()
            return Instance.new(createClassName)
        end)

        if not ok or not instance then
            warn("[Sync.createInstance] Failed to create CSG part " .. tostring(className))
            return nil
        end

        instance.Name = name

        -- Apply properties
        if data.properties then
            Sync.applyProperties(instance, data.properties)
        end

        -- Apply attributes
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

        -- Track by path for CSG reconstruction (don't parent yet)
        instancesByPath[data.path] = instance

        -- Cache for reference resolution
        if referenceId then
            instanceCache[referenceId] = instance
            Sync.registerInstanceByRefId(referenceId, instance)
        end

        return instance  -- Return but don't parent
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
                -- Note: We no longer store _rbxsync_ref on instances to avoid ScriptSync conflicts
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
        warn("[Sync.createInstance] Failed to create " .. tostring(className) .. ": " .. tostring(instance))
        return nil
    end

    -- Set name first (use the original name, not escaped/disambiguated)
    instance.Name = name

    -- Cache for reference resolution
    if referenceId then
        instanceCache[referenceId] = instance
        Sync.registerInstanceByRefId(referenceId, instance)
    end

    -- Note: We no longer store _rbxsync_ref on instances to avoid ScriptSync conflicts
    -- Reference resolution uses in-memory caches (instanceByRefId, instanceCache) only

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

    -- Track instance by path for CSG reconstruction
    if data.path then
        instancesByPath[data.path] = instance
        -- Mark path as recently applied for 50ms deduplication (prevents echo)
        ChangeTracker.markPathApplied(data.path)
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
        Capabilities = true,
        -- Other problematic properties
        CurrentCamera = true,
        TargetInstance = true,
        SourceInstance = true,
        UniqueId = true,
        HistoryId = true,
        SourceAssetId = true,
        ScriptGuid = true,
    }

    -- Content properties that need special handling (set via string-based property instead)
    -- Modern Roblox has both ColorMap (string) and ColorMapContent (Content)
    -- We'll set the string-based one and skip the Content one if it has no value
    local contentProps = {
        TextureContent = "Texture",
        EmissiveMaskContent = true,  -- No string equivalent, skip
        ColorMapContent = "ColorMap",
        NormalMapContent = "NormalMap",
        MetalnessMapContent = "MetalnessMap",
        RoughnessMapContent = "RoughnessMap",
        AudioContent = true,
        ImageContent = "Image",
        MoonTextureContent = "MoonTextureId",
        SunTextureContent = "SunTextureId",
        SkyboxUpContent = "SkyboxUp",
        SkyboxDownContent = "SkyboxDn",
        SkyboxLeftContent = "SkyboxLf",
        SkyboxRightContent = "SkyboxRt",
        SkyboxFrontContent = "SkyboxFt",
        SkyboxBackContent = "SkyboxBk",
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
            -- Check if this is an important property that should have worked
            local importantProps = {
                LightingStyle = true,  -- RobloxScriptSecurity - can read but not write
                Technology = true,     -- RobloxScriptSecurity - can read but not write
                BaseMaterial = true,   -- Important for MaterialVariant
                ColorMap = true,       -- Important for MaterialVariant
                NormalMap = true,      -- Important for MaterialVariant
                RoughnessMap = true,   -- Important for MaterialVariant
            }
            if importantProps[propName] then
                warn(string.format("[RbxSync] Cannot set %s.%s: %s (value type: %s)",
                    instance.ClassName, propName, tostring(err), typeof(value)))
            end
        else
            -- Debug: log successful MaterialVariant property sets
            if instance.ClassName == "MaterialVariant" and (propName == "BaseMaterial" or propName == "ColorMap") then
                print(string.format("[RbxSync Debug] Set MaterialVariant.%s = %s", propName, tostring(value)))
            end
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
    -- For CSG unions with separable data, skip CFrame to preserve geometry position
    if data.properties then
        local isCSGUnion = (data.className == "UnionOperation" or data.className == "IntersectOperation")
            and data.csg and data.csg.separable
        if isCSGUnion then
            -- Filter out CFrame for CSG unions
            local propsWithoutCFrame = {}
            for propName, propValue in data.properties do
                if propName ~= "CFrame" then
                    propsWithoutCFrame[propName] = propValue
                end
            end
            Sync.applyProperties(instance, propsWithoutCFrame)
        else
            Sync.applyProperties(instance, data.properties)
        end
    end

    -- Update script source
    -- Check both data.source (top-level) and data.properties.Source.value (from server)
    if instance:IsA("LuaSourceContainer") then
        local newSource = data.source
        if not newSource and data.properties and data.properties.Source then
            newSource = data.properties.Source.value
        end
        if newSource then
            pcall(function()
                (instance :: any).Source = newSource
            end)
        end
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

    -- Cache for reference resolution AND register for lookup
    if data.referenceId then
        instanceCache[data.referenceId] = instance
        -- Register in the refId lookup cache (no attribute to avoid ScriptSync conflicts)
        -- This ensures reference properties (Part0, Part1, etc.) can resolve
        Sync.registerInstanceByRefId(data.referenceId, instance)
    end

    -- Queue material overrides for MaterialService (apply after all MaterialVariants exist)
    if instance.ClassName == "MaterialService" and data.materialOverrides then
        for materialName, variantName in data.materialOverrides do
            table.insert(pendingMaterialOverrides, {
                materialName = materialName,
                variantName = variantName,
            })
        end
        print(string.format("[RbxSync] Queued %d material overrides for later application", #pendingMaterialOverrides))
    end

    -- Mark path as recently applied for 50ms deduplication (prevents echo)
    if data.path then
        ChangeTracker.markPathApplied(data.path)
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
        -- Mark path as recently applied for 50ms deduplication (prevents echo)
        ChangeTracker.markPathApplied(path)
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

    -- Check if this is a CSG union with separable data
    -- For these, we skip CFrame comparison because the geometry defines position
    local isCSGUnion = (data.className == "UnionOperation" or data.className == "IntersectOperation")
        and data.csg and data.csg.separable

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

            -- Skip CFrame for CSG unions - geometry defines position
            if isCSGUnion and propName == "CFrame" then
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
            if attrName == SOURCE_ATTR then
                continue -- Skip our internal source tracking attribute
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

    -- Check for materialOverrides (MaterialService only)
    if data.materialOverrides and instance.ClassName == "MaterialService" then
        table.insert(reasons, "materialOverrides")
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
    table.clear(pendingMaterialOverrides)
    table.clear(pendingCSGReconstructions)
    table.clear(instancesByPath)
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
        -- Check caches first, then fall back to full search by attribute
        local target = instanceCache[refId] or instanceByRefId[refId] or Sync.findInstanceByRefId(refId)
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

-- Apply pending material overrides
-- Call this AFTER all MaterialVariants have been created
-- Uses SetBaseMaterialOverride(material, variantName) which takes a string name
function Sync.applyPendingMaterialOverrides(): number
    if #pendingMaterialOverrides == 0 then
        return 0
    end

    local MaterialService = game:GetService("MaterialService")
    local applied = 0
    local failed = 0

    print(string.format("[RbxSync] Applying %d material overrides...", #pendingMaterialOverrides))

    for _, pending in pendingMaterialOverrides do
        local materialName = pending.materialName
        local variantName = pending.variantName

        -- Skip if variant name equals material name (no custom override)
        if variantName == materialName then
            continue
        end

        -- Find the material enum
        local ok, material = pcall(function()
            return (Enum.Material :: any)[materialName]
        end)

        if not ok or not material then
            warn(string.format("[RbxSync] Unknown material: %s", materialName))
            failed += 1
            continue
        end

        -- Verify the MaterialVariant exists
        local variant = MaterialService:FindFirstChild(variantName)
        if not variant then
            warn(string.format("[RbxSync] MaterialVariant not found: %s", variantName))
            failed += 1
            continue
        end

        -- Apply the override using SetBaseMaterialOverride (takes string name, not Instance)
        local setOk, err = pcall(function()
            MaterialService:SetBaseMaterialOverride(material, variantName)
        end)

        if setOk then
            applied += 1
            print(string.format("[RbxSync] Set material override: %s -> %s", materialName, variantName))
        else
            warn(string.format("[RbxSync] Failed to set override %s -> %s: %s", materialName, variantName, tostring(err)))
            failed += 1
        end
    end

    table.clear(pendingMaterialOverrides)
    print(string.format("[RbxSync] Material overrides complete: %d applied, %d failed", applied, failed))
    return applied
end

-- Reconstruct pending CSG operations (unions/intersections)
-- Call this AFTER all component parts have been created
function Sync.reconstructPendingCSG(): number
    if #pendingCSGReconstructions == 0 then
        return 0
    end

    print(string.format("[RbxSync] Reconstructing %d CSG operations...", #pendingCSGReconstructions))

    local reconstructed = 0
    local failed = 0

    for _, pending in pendingCSGReconstructions do
        local data = pending.data
        local parent = pending.parent
        local csgData = data.csg

        -- Collect component parts by path
        local basePath = pending.path .. "/_csg/"
        local addParts: {BasePart} = {}
        local subtractParts: {BasePart} = {}
        local missingParts = 0

        for _, op in csgData.operations do
            local partPath = basePath .. op.partRef
            local part = instancesByPath[partPath]

            if part and part:IsA("BasePart") then
                if op.type == "add" then
                    table.insert(addParts, part :: BasePart)
                elseif op.type == "subtract" then
                    table.insert(subtractParts, part :: BasePart)
                end
            else
                missingParts += 1
                warn(string.format("[RbxSync] CSG part not found: %s", partPath))
            end
        end

        if missingParts > 0 then
            warn(string.format("[RbxSync] Missing %d parts for union '%s', skipping", missingParts, data.name))
            failed += 1
            continue
        end

        if #addParts == 0 then
            warn(string.format("[RbxSync] No add parts for union '%s', skipping", data.name))
            failed += 1
            continue
        end

        -- CSG APIs require parts to be in the DataModel
        -- Create a temporary folder and parent all parts there
        local tempFolder = Instance.new("Folder")
        tempFolder.Name = "_RbxSync_CSG_Temp"
        tempFolder.Parent = game:GetService("ServerStorage")

        -- Parent all parts to the temp folder
        for _, part in addParts do
            part.Parent = tempFolder
        end
        for _, part in subtractParts do
            part.Parent = tempFolder
        end

        -- Reconstruct the union using CSGHandler
        local basePart = table.remove(addParts, 1) :: BasePart
        local success, union = CSGHandler.reconstruct(basePart, addParts, subtractParts)

        if success and union then
            -- Apply original properties, but skip CFrame since the geometry
            -- is already correctly positioned from the component parts.
            -- Applying CFrame would double-transform the union.
            if data.properties then
                local propsWithoutCFrame = {}
                for propName, propValue in data.properties do
                    if propName ~= "CFrame" then
                        propsWithoutCFrame[propName] = propValue
                    end
                end
                Sync.applyProperties(union, propsWithoutCFrame)
            end

            -- Set name and parent
            union.Name = data.name
            union.Parent = parent

            -- Apply attributes
            if data.attributes then
                for attrName, attrData in data.attributes do
                    if string.sub(attrName, 1, 4) == "RBX_" then
                        continue
                    end
                    local attrValue = decodeValue(attrData)
                    if attrValue ~= nil then
                        pcall(function()
                            union:SetAttribute(attrName, attrValue)
                        end)
                    end
                end
            end

            -- Apply tags
            if data.tags then
                for _, tag in data.tags do
                    CollectionService:AddTag(union, tag)
                end
            end

            -- Set source tracking
            if data.path then
                pcall(function()
                    union:SetAttribute(SOURCE_ATTR, data.path)
                end)
            end

            -- Cache for reference resolution (no attribute to avoid ScriptSync conflicts)
            if data.referenceId then
                instanceCache[data.referenceId] = union
                Sync.registerInstanceByRefId(data.referenceId, union)
            end

            -- Track by path
            instancesByPath[data.path] = union

            -- Mark path as recently applied for 50ms deduplication (prevents echo)
            ChangeTracker.markPathApplied(data.path)

            -- Clean up the component parts (they're now part of the union)
            basePart:Destroy()
            for _, part in addParts do
                part:Destroy()
            end
            for _, part in subtractParts do
                part:Destroy()
            end

            -- Clean up the temp folder
            tempFolder:Destroy()

            -- Also remove CSG parts from instancesByPath
            for _, op in csgData.operations do
                local partPath = basePath .. op.partRef
                instancesByPath[partPath] = nil
            end

            reconstructed += 1
            print(string.format("[RbxSync] Reconstructed union '%s'", data.name))
        else
            warn(string.format("[RbxSync] Failed to reconstruct union '%s'", data.name))
            -- Clean up temp folder on failure too
            tempFolder:Destroy()
            failed += 1
        end
    end

    table.clear(pendingCSGReconstructions)
    print(string.format("[RbxSync] CSG reconstruction complete: %d reconstructed, %d failed", reconstructed, failed))
    return reconstructed
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
            local descendants = service:GetDescendants()
            for i, descendant in descendants do
                -- Use full name as cache key for now
                local fullName = descendant:GetFullName()
                instanceCache[fullName] = descendant

                -- Yield periodically to avoid script timeout on large games (RBXSYNC-25)
                if i % 5000 == 0 then
                    task.wait()
                end
            end
        end
    end
end

return Sync
