--!strict
--[[
    Serializer Module

    Converts Roblox instances to JSON-serializable format with ALL properties.
    Uses reflection to read every property defined in the API dump.
]]

local HttpService = game:GetService("HttpService")
local CollectionService = game:GetService("CollectionService")

local Reflection = require(script.Parent.Reflection)

-- Type aliases
type APIDump = Reflection.APIDump
type PropertyInfo = Reflection.PropertyInfo

-- Cache for disambiguated paths (to handle duplicate sibling names)
local instancePaths: {[Instance]: string} = {}

local Serializer = {}

-- Escape special characters in instance names for use in paths
-- "/" is our path delimiter, so we must escape it
local function escapeNameForPath(name: string): string
    -- Replace / with a safe escape sequence that's unlikely to appear naturally
    -- Using [SLASH] as it's readable and very unlikely to be in a real name
    -- Note: Wrap in parentheses to return only the first value (string.gsub returns 2 values)
    return (string.gsub(name, "/", "[SLASH]"))
end

-- Unescape path segment back to original name
function Serializer.unescapePathSegment(segment: string): string
    return string.gsub(segment, "%[SLASH%]", "/")
end

-- Build disambiguated paths for all instances in a subtree
-- This handles duplicate sibling names by appending a suffix
-- rootPath should be the path to the root instance (e.g., "Workspace" for game.Workspace)
function Serializer.buildDisambiguatedPaths(root: Instance, rootPath: string?)
    local startPath = rootPath or escapeNameForPath(root.Name)

    -- Also store path for the root itself
    instancePaths[root] = startPath

    -- Counter for yielding to avoid script timeout on large games (RBXSYNC-25)
    local processedCount = 0

    local function buildPathsRecursive(instance: Instance, parentPath: string)
        -- Group children by name to find duplicates
        local children = instance:GetChildren()
        local nameCount: {[string]: number} = {}
        local nameSeen: {[string]: number} = {}

        -- First pass: count occurrences of each name
        for _, child in children do
            local name = child.Name
            nameCount[name] = (nameCount[name] or 0) + 1
        end

        -- Second pass: assign paths with disambiguation
        for _, child in children do
            local name = child.Name
            local escapedName = escapeNameForPath(name)
            local childPath: string

            if nameCount[name] > 1 then
                -- Duplicate name - use short ID suffix for disambiguation
                nameSeen[name] = (nameSeen[name] or 0) + 1
                local index = nameSeen[name]
                if index == 1 then
                    -- First occurrence keeps original name
                    childPath = parentPath .. "/" .. escapedName
                else
                    -- Subsequent occurrences get short debugId suffix (8 chars)
                    local debugId = child:GetDebugId(0)
                    local shortId = string.sub(debugId, 1, 8)
                    childPath = parentPath .. "/" .. escapedName .. "~" .. shortId
                end
            else
                -- Unique name - use as is
                childPath = parentPath .. "/" .. escapedName
            end

            instancePaths[child] = childPath

            -- Yield periodically to avoid script timeout on large games (RBXSYNC-25)
            processedCount += 1
            if processedCount % 5000 == 0 then
                task.wait()
            end

            -- Recurse to children
            buildPathsRecursive(child, childPath)
        end
    end

    -- Start from root, using its path as the prefix for children
    buildPathsRecursive(root, startPath)
end

-- Get unique ID for an instance using Roblox's built-in debug ID
-- GetDebugId(0) returns a session-scoped unique identifier that is:
-- - Stable across property changes, reparenting, and undo/redo
-- - Unique across all instances within a session
-- - Simpler than custom UUID generation (RBXSYNC-36)
local function getInstanceUUID(instance: Instance): string
    return instance:GetDebugId(0)
end

-- Encode a value to our JSON format
local function encodeValue(value: any, typeName: string): any
    if value == nil then
        return { type = "nil", value = nil }
    end

    -- Handle primitive types
    if typeName == "bool" or typeName == "boolean" then
        return { type = "bool", value = value }
    elseif typeName == "int" or typeName == "int64" or typeName == "number" then
        -- Check if value is actually a number
        if type(value) == "number" then
            if value == math.floor(value) then
                return { type = "int", value = value }
            else
                return { type = "float", value = value }
            end
        end
        return { type = "int", value = value }
    elseif typeName == "float" or typeName == "double" then
        -- Handle special float values
        if value ~= value then -- NaN
            return { type = typeName, value = "NaN" }
        elseif value == math.huge then
            return { type = typeName, value = "Infinity" }
        elseif value == -math.huge then
            return { type = typeName, value = "-Infinity" }
        end
        return { type = typeName, value = value }
    elseif typeName == "string" or typeName == "ProtectedString" then
        return { type = typeName, value = value }
    elseif typeName == "Content" then
        -- Content is a special type that wraps asset references
        -- Extract the URI if available
        if value and typeof(value) == "table" or typeof(value) == "userdata" then
            local uri = nil
            pcall(function()
                uri = value.Uri
            end)
            if uri and uri ~= "" then
                return { type = "Content", value = uri }
            end
        end
        -- Fallback: if value is already a string (older API), use it directly
        if type(value) == "string" then
            return { type = "Content", value = value }
        end
        return { type = "Content", value = nil }
    end

    -- If value is a number but type wasn't recognized, encode as number
    if type(value) == "number" then
        if value == math.floor(value) then
            return { type = "int", value = value }
        else
            return { type = "float", value = value }
        end
    end

    -- Handle Vector types
    if typeName == "Vector2" then
        return { type = "Vector2", value = { x = value.X, y = value.Y } }
    elseif typeName == "Vector2int16" then
        return { type = "Vector2int16", value = { x = value.X, y = value.Y } }
    elseif typeName == "Vector3" then
        return { type = "Vector3", value = { x = value.X, y = value.Y, z = value.Z } }
    elseif typeName == "Vector3int16" then
        return { type = "Vector3int16", value = { x = value.X, y = value.Y, z = value.Z } }
    end

    -- Handle CFrame
    if typeName == "CFrame" then
        local components = { value:GetComponents() }
        return {
            type = "CFrame",
            value = {
                position = { components[1], components[2], components[3] },
                rotation = {
                    components[4], components[5], components[6],
                    components[7], components[8], components[9],
                    components[10], components[11], components[12]
                }
            }
        }
    end

    -- Handle Color types
    if typeName == "Color3" then
        return { type = "Color3", value = { r = value.R, g = value.G, b = value.B } }
    elseif typeName == "Color3uint8" then
        return {
            type = "Color3uint8",
            value = {
                r = math.floor(value.R * 255),
                g = math.floor(value.G * 255),
                b = math.floor(value.B * 255)
            }
        }
    elseif typeName == "BrickColor" then
        return { type = "BrickColor", value = value.Number }
    end

    -- Handle UDim types
    if typeName == "UDim" then
        return { type = "UDim", value = { scale = value.Scale, offset = value.Offset } }
    elseif typeName == "UDim2" then
        return {
            type = "UDim2",
            value = {
                x = { scale = value.X.Scale, offset = value.X.Offset },
                y = { scale = value.Y.Scale, offset = value.Y.Offset }
            }
        }
    end

    -- Handle Rect
    if typeName == "Rect" then
        return {
            type = "Rect",
            value = {
                min = { x = value.Min.X, y = value.Min.Y },
                max = { x = value.Max.X, y = value.Max.Y }
            }
        }
    end

    -- Handle NumberSequence
    if typeName == "NumberSequence" then
        local keypoints = {}
        for _, kp in value.Keypoints do
            table.insert(keypoints, {
                time = kp.Time,
                value = kp.Value,
                envelope = kp.Envelope
            })
        end
        return { type = "NumberSequence", value = { keypoints = keypoints } }
    end

    -- Handle ColorSequence
    if typeName == "ColorSequence" then
        local keypoints = {}
        for _, kp in value.Keypoints do
            table.insert(keypoints, {
                time = kp.Time,
                color = { r = kp.Value.R, g = kp.Value.G, b = kp.Value.B }
            })
        end
        return { type = "ColorSequence", value = { keypoints = keypoints } }
    end

    -- Handle NumberRange
    if typeName == "NumberRange" then
        return { type = "NumberRange", value = { min = value.Min, max = value.Max } }
    end

    -- Handle Font (must be before Enum check since typeof(Font) is not "EnumItem")
    if typeName == "Font" then
        return {
            type = "Font",
            value = {
                family = value.Family,
                weight = tostring(value.Weight):match("%.(%w+)$") or tostring(value.Weight),
                style = tostring(value.Style):match("%.(%w+)$") or tostring(value.Style)
            }
        }
    end

    -- Handle Enum - use typeof() to properly detect EnumItem values
    if typeof(value) == "EnumItem" or string.match(typeName, "^Enum.") then
        -- Extract enum type and value
        local enumStr = tostring(value)
        local enumType = string.match(enumStr, "^Enum%.(%w+)%.")
        local enumValue = string.match(enumStr, "%.([^%.]+)$")
        return {
            type = "Enum",
            value = {
                enumType = enumType or typeName,
                value = enumValue or enumStr
            }
        }
    end

    -- Handle Instance references
    if typeName == "Instance" or typeof(value) == "Instance" then
        if value then
            return { type = "Ref", value = getInstanceUUID(value) }
        else
            return { type = "Ref", value = nil }
        end
    end

    -- Handle Faces
    if typeName == "Faces" then
        return {
            type = "Faces",
            value = {
                top = value.Top,
                bottom = value.Bottom,
                left = value.Left,
                right = value.Right,
                front = value.Front,
                back = value.Back
            }
        }
    end

    -- Handle Axes
    if typeName == "Axes" then
        return {
            type = "Axes",
            value = {
                x = value.X,
                y = value.Y,
                z = value.Z
            }
        }
    end

    -- Handle PhysicalProperties
    if typeName == "PhysicalProperties" then
        if value then
            return {
                type = "PhysicalProperties",
                value = {
                    density = value.Density,
                    friction = value.Friction,
                    elasticity = value.Elasticity,
                    frictionWeight = value.FrictionWeight,
                    elasticityWeight = value.ElasticityWeight
                }
            }
        else
            return { type = "PhysicalProperties", value = nil }
        end
    end

    -- Handle Ray
    if typeName == "Ray" then
        return {
            type = "Ray",
            value = {
                origin = { x = value.Origin.X, y = value.Origin.Y, z = value.Origin.Z },
                direction = { x = value.Direction.X, y = value.Direction.Y, z = value.Direction.Z }
            }
        }
    end

    -- Handle Region3
    if typeName == "Region3" then
        local cframe = value.CFrame
        local size = value.Size
        return {
            type = "Region3",
            value = {
                min = { x = cframe.X - size.X/2, y = cframe.Y - size.Y/2, z = cframe.Z - size.Z/2 },
                max = { x = cframe.X + size.X/2, y = cframe.Y + size.Y/2, z = cframe.Z + size.Z/2 }
            }
        }
    end

    -- Handle Region3int16
    if typeName == "Region3int16" then
        return {
            type = "Region3int16",
            value = {
                min = { x = value.Min.X, y = value.Min.Y, z = value.Min.Z },
                max = { x = value.Max.X, y = value.Max.Y, z = value.Max.Z }
            }
        }
    end

    -- Handle OptionalCoordinateFrame
    if typeName == "OptionalCoordinateFrame" then
        if value then
            local components = { value:GetComponents() }
            return {
                type = "OptionalCFrame",
                value = {
                    position = { components[1], components[2], components[3] },
                    rotation = {
                        components[4], components[5], components[6],
                        components[7], components[8], components[9],
                        components[10], components[11], components[12]
                    }
                }
            }
        else
            return { type = "OptionalCFrame", value = nil }
        end
    end

    -- Handle binary data (base64 encode)
    if typeName == "BinaryString" or typeName == "SharedString" then
        -- Note: We can't directly read BinaryString in Luau
        return { type = "BinaryString", value = "" }
    end

    -- Handle SecurityCapabilities
    if typeName == "SecurityCapabilities" then
        return { type = "SecurityCapabilities", value = 0 }
    end

    -- Handle UniqueId
    if typeName == "UniqueId" then
        return { type = "UniqueId", value = tostring(value) }
    end

    -- Fallback: try to convert to string
    local ok, str = pcall(function()
        return tostring(value)
    end)

    if ok then
        return { type = "string", value = str, originalType = typeName }
    end

    return { type = "unknown", value = nil, originalType = typeName }
end

-- Serialize a single instance
function Serializer.serializeInstance(instance: Instance, apiDump: APIDump): any?
    local className = instance.ClassName

    -- Get all properties for this class
    local properties = Reflection.getPropertiesForClass(className)

    -- Build properties map
    local propsMap = {}
    local errorCount = 0

    for _, propInfo in properties do
        if Reflection.shouldSerializeProperty(propInfo) then
            local propName = propInfo.Name
            local typeName = Reflection.getValueTypeName(propInfo)

            -- Try to read the property
            local ok, value = pcall(function()
                return (instance :: any)[propName]
            end)

            if ok then
                local encoded = encodeValue(value, typeName)
                if encoded then
                    propsMap[propName] = encoded
                end
            else
                errorCount += 1
                if errorCount <= 3 then
                    -- Only warn for first few errors to avoid spam
                    -- warn(string.format("[RbxSync] Could not read %s.%s: %s", className, propName, tostring(value)))
                end
            end
        end
    end

    -- Get attributes
    local attributes = {}
    local ok, attrs = pcall(function()
        return instance:GetAttributes()
    end)

    if ok and attrs then
        for name, value in attrs do
            local attrType = typeof(value)
            attributes[name] = encodeValue(value, attrType)
        end
    end

    -- Get tags
    local tags = CollectionService:GetTags(instance)

    -- Get parent info
    local parentId = nil
    -- Use cached disambiguated path if available, otherwise build from names
    local path = instancePaths[instance]
    if not path then
        -- Fallback: Build path manually with escaped names
        local pathParts = {}
        local current: Instance? = instance
        while current and current ~= game do
            table.insert(pathParts, 1, escapeNameForPath(current.Name))
            current = current.Parent
        end
        path = table.concat(pathParts, "/")
    end
    if instance.Parent then
        parentId = getInstanceUUID(instance.Parent)
    end

    -- Build serialized instance
    local serialized: any = {
        className = className,
        name = instance.Name,
        referenceId = getInstanceUUID(instance),
        parentId = parentId,
        path = path,
        properties = propsMap,
    }

    -- Only include non-empty collections
    if next(attributes) then
        serialized.attributes = attributes
    end

    if #tags > 0 then
        serialized.tags = tags
    end

    -- Note: Script source is NOT included in rbxjson
    -- The .luau file is the single source of truth for script code

    -- Special handling for MaterialService: extract material override assignments
    -- Uses GetBaseMaterialOverride which returns the MaterialVariant name as a string
    if className == "MaterialService" then
        local MaterialService = instance :: MaterialService
        local materialOverrides = {}

        -- Get all materials that can be overridden
        for _, material in Enum.Material:GetEnumItems() do
            local ok, overrideName = pcall(function()
                return MaterialService:GetBaseMaterialOverride(material)
            end)
            -- GetBaseMaterialOverride returns empty string if no override
            if ok and overrideName and overrideName ~= "" then
                materialOverrides[material.Name] = overrideName
            end
        end

        if next(materialOverrides) then
            serialized.materialOverrides = materialOverrides
            print(string.format("[RbxSync] Extracted %d material overrides from MaterialService",
                (function() local c = 0 for _ in materialOverrides do c += 1 end return c end)()))
        end
    end

    return serialized
end

-- Clear path cache (call between extractions)
function Serializer.clearCache()
    table.clear(instancePaths)
end

-- Get the path for an instance (Fixes RBXSYNC-80)
-- Uses cached disambiguated path if available, otherwise builds from names
function Serializer.getPath(instance: Instance): string
    -- Check cache first
    local path = instancePaths[instance]
    if path then
        return path
    end

    -- Fallback: Build path manually with escaped names
    local pathParts = {}
    local current: Instance? = instance
    while current and current ~= game do
        table.insert(pathParts, 1, escapeNameForPath(current.Name))
        current = current.Parent
    end
    return table.concat(pathParts, "/")
end

return Serializer
