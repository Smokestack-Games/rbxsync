--!strict
--[[
    CSG Handler

    Handles UnionOperation, NegateOperation, and IntersectOperation instances.
    Uses Plugin:Separate() to deconstruct unions for extraction,
    and UnionAsync/SubtractAsync to reconstruct them during sync.
]]

export type CSGData = {
    className: string,
    assetId: string?,
    usePartColor: boolean,
    smoothingAngle: number,
    hasCollision: boolean,
}

export type SeparatedCSG = {
    success: boolean,
    addParts: {BasePart},
    subtractParts: {BasePart},
    error: string?,
}

export type CSGOperation = {
    type: "add" | "subtract",
    partRef: string,
}

export type CSGMetadata = {
    version: number,
    separable: boolean,
    operations: {CSGOperation},
}

local CSGHandler = {}

-- Check if an instance is a CSG operation
function CSGHandler.isCSGOperation(instance: Instance): boolean
    local className = instance.ClassName
    return className == "UnionOperation"
        or className == "NegateOperation"
        or className == "IntersectOperation"
end

-- Extract CSG data from an operation
function CSGHandler.extractCSGData(instance: Instance): CSGData?
    if not CSGHandler.isCSGOperation(instance) then
        return nil
    end

    local className = instance.ClassName

    -- Try to get AssetId (may not always be available)
    local assetId: string? = nil
    local ok, id = pcall(function()
        return (instance :: any).AssetId
    end)
    if ok and id and id ~= "" then
        assetId = id
    end

    -- Get common properties
    local usePartColor = false
    local smoothingAngle = 0
    local hasCollision = true

    pcall(function()
        usePartColor = (instance :: any).UsePartColor
    end)

    pcall(function()
        smoothingAngle = (instance :: any).SmoothingAngle
    end)

    pcall(function()
        -- Check if collision is enabled via CanCollide
        hasCollision = (instance :: any).CanCollide
    end)

    return {
        className = className,
        assetId = assetId,
        usePartColor = usePartColor,
        smoothingAngle = smoothingAngle,
        hasCollision = hasCollision,
    }
end

-- Separate a UnionOperation into its component parts using Plugin:Separate()
-- Returns the separated parts categorized as "add" or "subtract" operations
function CSGHandler.separate(pluginInstance: Plugin, union: BasePart): SeparatedCSG
    if not union:IsA("UnionOperation") and not union:IsA("IntersectOperation") then
        return {
            success = false,
            addParts = {},
            subtractParts = {},
            error = "Instance is not a UnionOperation or IntersectOperation",
        }
    end

    -- Call Plugin:Separate() to deconstruct the union
    local ok, results = pcall(function()
        return pluginInstance:Separate({union})
    end)

    if not ok then
        return {
            success = false,
            addParts = {},
            subtractParts = {},
            error = "Plugin:Separate() failed: " .. tostring(results),
        }
    end

    if not results or #results == 0 then
        return {
            success = false,
            addParts = {},
            subtractParts = {},
            error = "Plugin:Separate() returned no parts",
        }
    end

    local addParts: {BasePart} = {}
    local subtractParts: {BasePart} = {}

    for _, part in results do
        if part:IsA("NegateOperation") then
            -- This is a subtracted part wrapped in NegateOperation
            -- The actual geometry is inside the NegateOperation
            -- NegateOperation itself is a BasePart, so we keep it as-is
            -- When reconstructing, we'll use SubtractAsync with these
            table.insert(subtractParts, part :: BasePart)
        elseif part:IsA("BasePart") then
            -- Regular part that was added to the union
            table.insert(addParts, part :: BasePart)
        end
    end

    return {
        success = true,
        addParts = addParts,
        subtractParts = subtractParts,
        error = nil,
    }
end

-- Reconstruct a union from separated parts
-- basePart is the first part, additionalParts are unioned to it
-- subtractParts are subtracted from the result
function CSGHandler.reconstruct(
    basePart: BasePart,
    additionalParts: {BasePart},
    subtractParts: {BasePart}
): (boolean, BasePart?)
    local result: BasePart = basePart

    -- First, union all the add parts together
    if #additionalParts > 0 then
        local ok, unionResult = pcall(function()
            return basePart:UnionAsync(additionalParts)
        end)

        if not ok then
            warn("[CSGHandler] UnionAsync failed:", unionResult)
            return false, nil
        end

        result = unionResult :: BasePart
    end

    -- Then subtract the subtract parts
    if #subtractParts > 0 then
        local ok, subtractResult = pcall(function()
            return result:SubtractAsync(subtractParts)
        end)

        if not ok then
            warn("[CSGHandler] SubtractAsync failed:", subtractResult)
            -- Clean up the intermediate union if we created one
            if result ~= basePart then
                result:Destroy()
            end
            return false, nil
        end

        -- Clean up the intermediate union
        if result ~= basePart then
            result:Destroy()
        end

        result = subtractResult :: BasePart
    end

    return true, result
end

return CSGHandler
