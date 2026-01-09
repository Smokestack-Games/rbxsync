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

    -- Check if property is serializable (CanSave)
    -- This excludes computed/read-only properties like Mass, AssemblyCenterOfMass, etc.
    if prop.Serialization and prop.Serialization.CanSave == false then
        return false
    end

    -- Check write security - only include properties we can actually modify
    local writeSecurity = prop.Security and prop.Security.Write
    if writeSecurity and writeSecurity ~= "None" and writeSecurity ~= "PluginSecurity" then
        return false
    end

    -- Check for tags that indicate non-editable properties
    if prop.Tags then
        for _, tag in prop.Tags do
            if tag == "ReadOnly" or tag == "NotScriptable" or tag == "Hidden" or tag == "Deprecated" then
                return false
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

return Reflection
