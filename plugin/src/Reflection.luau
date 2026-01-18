--!strict
--[[
    Reflection Module

    Loads the Roblox API dump and provides property information for any class.
    This enables reading ALL properties of any instance, not just commonly known ones.
]]

local HttpService = game:GetService("HttpService")

-- API dump URL (from Roblox Client Tracker)
local API_DUMP_URL = "https://raw.githubusercontent.com/MaximumADHD/Roblox-Client-Tracker/roblox/API-Dump.json"

-- Types
export type PropertyInfo = {
    Name: string,
    ValueType: {Category: string, Name: string},
    Security: {Read: string, Write: string},
    Serialization: {CanSave: boolean, CanLoad: boolean},
    Tags: {string}?,
}

export type ClassInfo = {
    Name: string,
    Superclass: string?,
    Members: {PropertyInfo},
    Tags: {string}?,
}

export type APIDump = {
    Classes: {ClassInfo},
    Enums: {any},
    Version: number,
}

-- Cached API dump
local cachedDump: APIDump? = nil
local classPropertyCache: {[string]: {PropertyInfo}} = {}
local classSuperclassMap: {[string]: string?} = {}

local Reflection = {}

-- Load API dump from GitHub
function Reflection.loadAPIDump(): APIDump?
    if cachedDump then
        return cachedDump
    end

    print("[RbxSync] Fetching API dump...")

    local success, result = pcall(function()
        return HttpService:GetAsync(API_DUMP_URL)
    end)

    if not success then
        warn("[RbxSync] Failed to fetch API dump:", result)

        -- Try to use a fallback embedded dump
        return Reflection.getFallbackDump()
    end

    local ok, dump = pcall(function()
        return HttpService:JSONDecode(result)
    end)

    if not ok then
        warn("[RbxSync] Failed to parse API dump:", dump)
        return nil
    end

    cachedDump = dump :: APIDump

    -- Build lookup tables
    for _, class in cachedDump.Classes do
        classSuperclassMap[class.Name] = class.Superclass

        -- Extract properties from members
        local properties: {PropertyInfo} = {}
        if class.Members then
            for _, member in class.Members do
                if member.MemberType == "Property" then
                    table.insert(properties, member :: PropertyInfo)
                end
            end
        end
        classPropertyCache[class.Name] = properties
    end

    print("[RbxSync] API dump loaded:", #cachedDump.Classes, "classes")

    return cachedDump
end

-- Get fallback embedded dump (minimal version for offline use)
function Reflection.getFallbackDump(): APIDump?
    -- This would contain a minimal embedded dump for common classes
    -- For now, return nil to indicate we need the full dump
    warn("[RbxSync] No fallback API dump available")
    return nil
end

-- Get all properties for a class (including inherited)
function Reflection.getPropertiesForClass(className: string): {PropertyInfo}
    if not cachedDump then
        return {}
    end

    local allProperties: {PropertyInfo} = {}
    local seenNames: {[string]: boolean} = {}
    local currentClass: string? = className

    -- Walk up the inheritance chain
    while currentClass do
        local classProps = classPropertyCache[currentClass]
        if classProps then
            for _, prop in classProps do
                if not seenNames[prop.Name] then
                    seenNames[prop.Name] = true
                    table.insert(allProperties, prop)
                end
            end
        end
        currentClass = classSuperclassMap[currentClass]
    end

    return allProperties
end

-- Check if a property can be read (readable from an instance)
function Reflection.canReadProperty(prop: PropertyInfo): boolean
    -- Check security - allow None and PluginSecurity
    local readSecurity = prop.Security and prop.Security.Read
    if readSecurity and readSecurity ~= "None" and readSecurity ~= "PluginSecurity" then
        return false
    end

    -- Check for problematic tags
    if prop.Tags then
        for _, tag in prop.Tags do
            -- NotScriptable and Hidden mean we can't access them via scripts
            if tag == "NotScriptable" or tag == "Hidden" then
                return false
            end
        end
    end

    return true
end

-- Check if a property should be included in serialization
function Reflection.shouldSerializeProperty(prop: PropertyInfo): boolean
    if not Reflection.canReadProperty(prop) then
        return false
    end

    -- Important properties that MUST be serialized even if CanSave is false
    -- These are marked as CanSave=false in the API dump but are essential for sync
    local forceSerialize = {
        -- BasePart essentials
        Size = true,
        Position = true,
        Orientation = true,
        CFrame = true,
        Color = true,
        BrickColor = true,
        Material = true,
        Transparency = true,
        Reflectance = true,
        CanCollide = true,
        Anchored = true,
        Shape = true,
        -- MeshPart Content properties
        MeshId = true,
        TextureID = true,
        -- SpecialMesh/FileMesh Content properties
        MeshType = true,
        -- Sound Content properties
        SoundId = true,
        -- Image Content properties (ImageLabel, ImageButton, etc.)
        Image = true,
        -- Decal/Texture Content properties
        Texture = true,
        Face = true,
        -- ParticleEmitter/Beam/Trail Content properties
        -- (Texture already covered above)
        -- Animation Content properties
        AnimationId = true,
        -- Video Content properties
        Video = true,
        -- Sky Content properties
        SkyboxBk = true,
        SkyboxDn = true,
        SkyboxFt = true,
        SkyboxLf = true,
        SkyboxRt = true,
        SkyboxUp = true,
        MoonTextureId = true,
        SunTextureId = true,
        -- Script Source (handled separately but include for safety)
        Source = true,
        -- Light properties
        Brightness = true,
        Range = true,
        -- Lighting properties (RobloxScriptSecurity write, but we can read them)
        LightingStyle = true,
        Technology = true,
        -- SurfaceAppearance Content properties
        ColorMap = true,
        MetalnessMap = true,
        NormalMap = true,
        RoughnessMap = true,
        -- Instance reference properties (constraints, welds, etc.)
        -- Comprehensive list to ensure ALL references are captured

        -- Welds and basic constraints (WeldConstraint, Weld, Motor6D, Motor, etc.)
        Part0 = true,
        Part1 = true,
        C0 = true,  -- CFrame offset (Weld, Motor6D)
        C1 = true,  -- CFrame offset (Weld, Motor6D)

        -- Attachment-based constraints (RopeConstraint, SpringConstraint, HingeConstraint,
        -- BallSocketConstraint, CylindricalConstraint, PrismaticConstraint, UniversalConstraint,
        -- AlignPosition, AlignOrientation, VectorForce, Torque, LinearVelocity, AngularVelocity,
        -- LineForce, PlaneConstraint, TorsionSpringConstraint, RigidConstraint, PathfindingLink)
        Attachment0 = true,
        Attachment1 = true,

        -- Model/Assembly
        PrimaryPart = true,

        -- GUI adornments (BillboardGui, SurfaceGui, SelectionBox, Highlight,
        -- BoxHandleAdornment, ConeHandleAdornment, CylinderHandleAdornment, etc.)
        Adornee = true,

        -- Value objects
        Value = true,  -- ObjectValue, CFrameValue, etc.

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

        -- Script linking (deprecated but may exist in old games)
        LinkedSource = true,

        -- Beam/Trail (use Attachment0/Attachment1, already covered)
        -- NoCollisionConstraint (uses Part0/Part1, already covered)
    }

    -- Check if property is serializable (CanSave)
    -- Skip computed/read-only properties, but allow force-serialized ones
    if prop.Serialization and prop.Serialization.CanSave == false then
        if not forceSerialize[prop.Name] then
            return false
        end
    end

    -- Check write security - only include properties we can actually modify
    -- But allow force-serialized properties (like MeshId) even if not writable
    local writeSecurity = prop.Security and prop.Security.Write
    if writeSecurity and writeSecurity ~= "None" and writeSecurity ~= "PluginSecurity" then
        if not forceSerialize[prop.Name] then
            return false
        end
    end

    -- Check for tags that indicate non-editable properties
    if prop.Tags then
        for _, tag in prop.Tags do
            if tag == "ReadOnly" or tag == "NotScriptable" or tag == "Hidden" or tag == "Deprecated" then
                -- Still allow force-serialized properties
                if not forceSerialize[prop.Name] then
                    return false
                end
            end
        end
    end

    -- Skip certain properties that are auto-generated or internal
    local skipProps = {
        "Parent", -- Handled separately via hierarchy
        "Name", -- Stored at top level
        "ClassName", -- Stored at top level
        "className", -- Lowercase variant
        "Archivable", -- Usually true, rarely needed
        "DataCost", -- Deprecated/internal
        "RobloxLocked", -- Internal
    }

    for _, skip in skipProps do
        if prop.Name == skip then
            return false
        end
    end

    return true
end

-- Get the value type category for encoding
function Reflection.getValueTypeCategory(prop: PropertyInfo): string
    if prop.ValueType then
        return prop.ValueType.Category
    end
    return "Primitive"
end

-- Get the value type name
function Reflection.getValueTypeName(prop: PropertyInfo): string
    if prop.ValueType then
        return prop.ValueType.Name
    end
    return "unknown"
end

-- Get the API dump, loading it if not already loaded (Fixes RBXSYNC-81)
-- This is an alias for loadAPIDump for consistency with other modules
function Reflection.getAPIDump(): APIDump?
    return Reflection.loadAPIDump()
end

return Reflection
