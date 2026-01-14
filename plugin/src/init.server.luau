--!strict
--[[
    RbxSync Studio Plugin
    Copyright (c) 2026 Smokestack Games. All rights reserved.

    This software is proprietary. Unauthorized copying, modification,
    distribution, or use is strictly prohibited. See LICENSE for details.

    Extracts complete game DataModel with ALL properties using reflection.
    Communicates with local RbxSync server via HTTP.
    Features configurable project paths and git integration.
]]

local HttpService = game:GetService("HttpService")
local Selection = game:GetService("Selection")
local ChangeHistoryService = game:GetService("ChangeHistoryService")
local MarketplaceService = game:GetService("MarketplaceService")
local LogService = game:GetService("LogService")

-- Configuration defaults
local POLL_INTERVAL = 1 -- seconds
local CHUNK_SIZE = 200 -- instances per chunk (Roblox HttpService has 1MB limit)
local PLUGIN_VERSION = "1.1.0" -- Current plugin version (update on release)
local UPDATE_CHECK_INTERVAL = 86400 -- Check for updates once per 24 hours

-- Plugin script execution for MCP/AI code runner integration
-- These functions are standard Roblox plugin APIs used by developer tools
local executeCode = loadstring
local getEnvironment = getfenv
local setEnvironment = setfenv

-- Check if code execution is available (standard plugin capability)
if not executeCode then
    warn("[RbxSync] Code execution not available - run:code feature disabled")
end

-- Load modules (modules are children of this script in the bundled .rbxm)
local Reflection = require(script.Reflection)
local Serializer = require(script.Serializer)
local Sync = require(script.Sync)
local Config = require(script.Config)
local TestRunner = require(script.TestRunner)
local ChangeTracker = require(script.ChangeTracker)
local TerrainHandler = require(script.TerrainHandler)
local CSGHandler = require(script.CSGHandler)
local BotController = require(script.BotController)

-- Initialize config with plugin reference
Config.init(plugin)

-- Plugin state
local isConnected = false
local isExtracting = false
local isSyncing = false
local userDisconnected = true  -- Start disconnected - user must explicitly connect
local hasShownConnectPrompt = false  -- Track if we've shown the connect prompt
local cachedPlaceName: string? = nil
local lastExtractTime: number? = nil  -- Track last extraction time for estimates
local lastInstanceCount: number? = nil  -- Track last instance count
local hasLoggedLink: boolean = false  -- Prevent spam logging of "Linked to" messages
local hasShownPathMismatch: boolean = false  -- Prevent showing path mismatch dialog multiple times per session
local lastKnownVscodePath: string? = nil  -- Track VS Code workspace to detect when it connects

-- Unique session ID for this Studio instance (handles multiple unpublished places with PlaceId=0)
local SESSION_ID = HttpService:GenerateGUID(false)

-- Console capture state (for E2E testing mode)
local consoleCapture = {
    enabled = false,
    connection = nil :: RBXScriptConnection?,
    flushThread = nil :: thread?,
    pendingMessages = {} :: {{timestamp: string, message_type: string, message: string, source: string?}},
    lastFlush = 0,
    FLUSH_INTERVAL = 0.5,  -- Flush every 500ms
}

-- Sync direction settings
local syncSettings = {
    studioToFiles = false,  -- Auto-extract Studio changes to files
    filesToStudio = true,   -- Sync file changes to Studio (enabled for MCP workflow)
}

-- Operation timing state (for live timer display)
local operationState = {
    extractStartTime = nil :: number?,
    syncStartTime = nil :: number?,
    lastExtractDuration = nil :: number?,
    lastSyncDuration = nil :: number?,
    lastExtractCount = nil :: number?,
    lastSyncCount = nil :: number?,
    timerThread = nil :: thread?,
}

-- Update checker state
local updateAvailableVersion: string? = nil

-- Version comparison helper
local function isNewerVersion(latest: string, current: string): boolean
    local function parseVersion(v: string): (number, number, number)
        local major, minor, patch = v:match("(%d+)%.(%d+)%.(%d+)")
        return tonumber(major) or 0, tonumber(minor) or 0, tonumber(patch) or 0
    end
    local lMaj, lMin, lPat = parseVersion(latest)
    local cMaj, cMin, cPat = parseVersion(current)
    if lMaj ~= cMaj then return lMaj > cMaj end
    if lMin ~= cMin then return lMin > cMin end
    return lPat > cPat
end

-- Check for updates from GitHub releases
local function checkForUpdates()
    -- Only check once per 24 hours
    local lastCheck = Config.getLastUpdateCheck()
    local now = os.time()
    if now - lastCheck < UPDATE_CHECK_INTERVAL then
        -- Load cached update status
        updateAvailableVersion = Config.getUpdateAvailable()
        return
    end

    -- Check GitHub releases API (runs in background)
    task.spawn(function()
        local url = "https://api.github.com/repos/devmarissa/rbxsync/releases/latest"
        local success, response = pcall(function()
            return HttpService:GetAsync(url)
        end)

        if success then
            local ok, data = pcall(function()
                return HttpService:JSONDecode(response)
            end)

            if ok and data and data.tag_name then
                local latestVersion = data.tag_name:gsub("^v", "")
                if isNewerVersion(latestVersion, PLUGIN_VERSION) then
                    updateAvailableVersion = latestVersion
                    Config.setUpdateAvailable(latestVersion)
                    print("[RbxSync] Update available: v" .. latestVersion .. " (current: v" .. PLUGIN_VERSION .. ")")
                    print("[RbxSync] Download at: https://github.com/devmarissa/rbxsync/releases")
                else
                    updateAvailableVersion = nil
                    Config.setUpdateAvailable(nil)
                end
            end
        end

        Config.setLastUpdateCheck(now)
    end)
end

-- Forward declaration for UI update function (defined later)
local updateConnectionUI: () -> ()

-- Get actual place name from MarketplaceService (or fallback)
local function getPlaceName(): string
    if cachedPlaceName then
        return cachedPlaceName
    end

    local placeId = game.PlaceId
    if placeId == 0 then
        -- Unsaved place - use game.Name or "Untitled"
        return game.Name ~= "Place1" and game.Name or "Untitled Place"
    end

    -- Try to get the actual place name from MarketplaceService
    local success, info = pcall(function()
        return MarketplaceService:GetProductInfo(placeId, Enum.InfoType.Asset)
    end)

    if success and info and info.Name then
        cachedPlaceName = info.Name
        return info.Name
    end

    -- Fallback to game.Name
    return game.Name ~= "Place1" and game.Name or "Place " .. tostring(placeId)
end

-- Get current server URL from config
local function getServerUrl(): string
    return Config.getServerUrl()
end

-- Console capture functions
local function getMessageType(messageType: Enum.MessageType): string
    if messageType == Enum.MessageType.MessageError then
        return "error"
    elseif messageType == Enum.MessageType.MessageWarning then
        return "warn"
    else
        return "info"
    end
end

local function flushConsoleMessages()
    if #consoleCapture.pendingMessages == 0 then
        return
    end

    local messages = consoleCapture.pendingMessages
    consoleCapture.pendingMessages = {}

    local url = getServerUrl() .. "/console/push"
    local payload = HttpService:JSONEncode({
        messages = messages
    })

    pcall(function()
        HttpService:RequestAsync({
            Url = url,
            Method = "POST",
            Headers = { ["Content-Type"] = "application/json" },
            Body = payload,
        })
    end)
end

local function startConsoleCapture()
    if consoleCapture.enabled then
        return
    end

    consoleCapture.enabled = true
    consoleCapture.pendingMessages = {}
    consoleCapture.lastFlush = tick()

    -- Hook into LogService to capture all console output
    consoleCapture.connection = LogService.MessageOut:Connect(function(message, messageType)
        local now = os.date("%H:%M:%S")
        table.insert(consoleCapture.pendingMessages, {
            timestamp = now,
            message_type = getMessageType(messageType),
            message = message,
            source = "studio",
        })
    end)

    -- Start a periodic flush thread to ensure messages are sent even if no new ones arrive
    consoleCapture.flushThread = task.spawn(function()
        while consoleCapture.enabled do
            task.wait(consoleCapture.FLUSH_INTERVAL)
            if #consoleCapture.pendingMessages > 0 then
                flushConsoleMessages()
            end
        end
    end)

    print("[RbxSync] Console capture enabled")
end

local function stopConsoleCapture()
    if not consoleCapture.enabled then
        return
    end

    consoleCapture.enabled = false

    if consoleCapture.connection then
        consoleCapture.connection:Disconnect()
        consoleCapture.connection = nil
    end

    -- Cancel the flush thread
    if consoleCapture.flushThread then
        task.cancel(consoleCapture.flushThread)
        consoleCapture.flushThread = nil
    end

    -- Flush any remaining messages
    flushConsoleMessages()

    print("[RbxSync] Console capture disabled")
end

-- Format number with commas (1234 -> "1,234")
local function formatNumber(n: number): string
    local formatted = tostring(n)
    local k
    while true do
        formatted, k = string.gsub(formatted, "^(-?%d+)(%d%d%d)", '%1,%2')
        if k == 0 then break end
    end
    return formatted
end

-- Count instances by class in standard services
local function getInstanceBreakdown(): {total: number, classes: {[string]: number}, scripts: number}
    local servicesToCount = {
        "Workspace",
        "ReplicatedStorage",
        "ReplicatedFirst",
        "ServerScriptService",
        "ServerStorage",
        "StarterGui",
        "StarterPack",
        "StarterPlayer",
        "Lighting",
        "SoundService",
    }

    local total = 0
    local classes = {}
    local scripts = 0

    for _, serviceName in servicesToCount do
        local service = game:FindFirstChild(serviceName)
        if service then
            -- Count the service itself
            total += 1
            classes[service.ClassName] = (classes[service.ClassName] or 0) + 1

            -- Count descendants
            for _, descendant in service:GetDescendants() do
                total += 1
                if descendant:IsA("LuaSourceContainer") then
                    scripts += 1
                end
                classes[descendant.ClassName] = (classes[descendant.ClassName] or 0) + 1
            end
        end
    end

    return { total = total, classes = classes, scripts = scripts }
end

-- Get optimization-related stats (lights, animations, meshes, etc.)
local function getOptimizationStats(): {lights: number, animations: number, skinnedMeshes: number, sounds: number, particles: number, meshParts: number, unions: number}
    local servicesToCount = {
        "Workspace",
        "ReplicatedStorage",
        "ReplicatedFirst",
        "ServerScriptService",
        "ServerStorage",
        "StarterGui",
        "StarterPack",
        "StarterPlayer",
        "Lighting",
        "SoundService",
    }

    local stats = {
        lights = 0,
        animations = 0,
        skinnedMeshes = 0,
        sounds = 0,
        particles = 0,
        meshParts = 0,
        unions = 0,
    }

    for _, serviceName in servicesToCount do
        local service = game:FindFirstChild(serviceName)
        if service then
            for _, desc in service:GetDescendants() do
                if desc:IsA("Light") then
                    stats.lights += 1
                elseif desc:IsA("Animation") or desc:IsA("AnimationTrack") then
                    stats.animations += 1
                elseif desc:IsA("MeshPart") then
                    stats.meshParts += 1
                    -- Check for skinned mesh (has bone attachments or wrap layers)
                    if desc:FindFirstChildOfClass("Bone") or desc:FindFirstChildOfClass("WrapTarget") or desc:FindFirstChildOfClass("WrapLayer") then
                        stats.skinnedMeshes += 1
                    end
                elseif desc:IsA("Sound") then
                    stats.sounds += 1
                elseif desc:IsA("ParticleEmitter") or desc:IsA("Beam") or desc:IsA("Trail") then
                    stats.particles += 1
                elseif desc:IsA("UnionOperation") then
                    stats.unions += 1
                end
            end
        end
    end

    return stats
end

-- Get optimization stats display string
local function getOptimizationStatsString(): string
    local stats = getOptimizationStats()
    local parts = {}

    if stats.lights > 0 then table.insert(parts, formatNumber(stats.lights) .. " lights") end
    if stats.meshParts > 0 then table.insert(parts, formatNumber(stats.meshParts) .. " meshes") end
    if stats.skinnedMeshes > 0 then table.insert(parts, formatNumber(stats.skinnedMeshes) .. " skinned") end
    if stats.sounds > 0 then table.insert(parts, formatNumber(stats.sounds) .. " sounds") end
    if stats.particles > 0 then table.insert(parts, formatNumber(stats.particles) .. " particles") end
    if stats.unions > 0 then table.insert(parts, formatNumber(stats.unions) .. " unions") end
    if stats.animations > 0 then table.insert(parts, formatNumber(stats.animations) .. " anims") end

    if #parts == 0 then
        return ""
    end

    return table.concat(parts, " • ")
end

-- Get clean instance display string
local function getClassBreakdownString(): string
    local breakdown = getInstanceBreakdown()

    -- Clean format: "1,247 instances • 89 scripts"
    local display = formatNumber(breakdown.total) .. " instances"

    if breakdown.scripts > 0 then
        display = display .. " • " .. formatNumber(breakdown.scripts) .. " scripts"
    end

    return display
end

-- Legacy function for backward compatibility
local function countInstances(): number
    return getInstanceBreakdown().total
end

-- Estimate extraction time based on instance count and previous extractions
local function getExtractionEstimate(): string
    local instanceCount = countInstances()

    if lastExtractTime and lastInstanceCount and lastInstanceCount > 0 then
        -- Estimate based on previous extraction
        local timePerInstance = lastExtractTime / lastInstanceCount
        local estimatedTime = instanceCount * timePerInstance
        if estimatedTime < 5 then
            return string.format("%d instances (~%ds)", instanceCount, math.ceil(estimatedTime))
        elseif estimatedTime < 60 then
            return string.format("%d instances (~%ds)", instanceCount, math.ceil(estimatedTime))
        else
            return string.format("%d instances (~%.1fm)", instanceCount, estimatedTime / 60)
        end
    else
        -- Rough estimate: ~0.003s per instance (based on typical performance)
        local estimatedTime = instanceCount * 0.003
        if estimatedTime < 5 then
            return string.format("%d instances", instanceCount)
        elseif estimatedTime < 60 then
            return string.format("%d instances (~%ds)", instanceCount, math.ceil(estimatedTime))
        else
            return string.format("%d instances (~%.1fm)", instanceCount, estimatedTime / 60)
        end
    end
end

-- HTTP helpers
local function httpGet(endpoint: string): (boolean, any)
    local serverUrl = getServerUrl()
    local success, result = pcall(function()
        return HttpService:GetAsync(serverUrl .. endpoint)
    end)

    if success then
        -- Handle empty responses (204 No Content)
        if result == nil or result == "" or result == "null" then
            return true, nil
        end

        local ok, data = pcall(function()
            return HttpService:JSONDecode(result)
        end)
        if ok then
            return true, data
        else
            -- JSON decode failed but HTTP succeeded - treat as empty response
            return true, nil
        end
    end

    return false, result
end

local function httpPost(endpoint: string, data: any): (boolean, any)
    local serverUrl = getServerUrl()
    local success, result = pcall(function()
        local json = HttpService:JSONEncode(data)
        return HttpService:PostAsync(serverUrl .. endpoint, json, Enum.HttpContentType.ApplicationJson)
    end)

    if success then
        local ok, decoded = pcall(function()
            return HttpService:JSONDecode(result)
        end)
        return ok, decoded
    end

    return false, result
end

-- Send extraction chunk with retry logic for oversized chunks
local function sendChunk(sessionId: string, chunkIndex: number, totalChunks: number, instances: {any}, projectDir: string): boolean
    local success, result = httpPost("/extract/chunk", {
        session_id = sessionId,
        chunk_index = chunkIndex,
        total_chunks = totalChunks,
        data = instances,
        project_dir = projectDir,
    })

    if success then
        return true
    end

    -- Check if it's a size limit error
    if type(result) == "string" and result:find("too large") then
        -- Try sending instances one at a time (silent)
        for i, inst in ipairs(instances) do
            local singleSuccess, singleResult = httpPost("/extract/chunk", {
                session_id = sessionId,
                chunk_index = chunkIndex,
                total_chunks = totalChunks,
                data = {inst},
                project_dir = projectDir,
            })
            if not singleSuccess then
                if type(singleResult) == "string" and singleResult:find("too large") then
                    -- Skip oversized instance silently
                else
                    return false
                end
            end
        end
        return true
    end

    return false
end

-- Forward declaration for setStatus (defined in UI section)
local setStatus: (text: string, color: Color3?, duration: number?) -> () = function() end

-- Forward declarations for path mismatch functions (defined in UI section)
local checkPathMismatch: () -> (boolean, string?)
local showPathMismatchDialog: (vscodePath: string, onContinue: () -> (), onFixPath: () -> (), onCancel: () -> ()) -> ()

-- Extract full game
local function extractGame(config: {any})
    if isExtracting then
        return
    end

    -- Use project_dir from config if provided, otherwise fall back to configured dir
    local projectDir = config.project_dir or Config.getProjectDir()
    if projectDir == "" then
        warn("No project set. Enter path above or pass project_dir in command.")
        return
    end

    isExtracting = true
    local startTime = os.clock()
    operationState.extractStartTime = startTime

    -- Start live timer update in status
    local timerThread = task.spawn(function()
        while operationState.extractStartTime do
            local elapsed = os.clock() - startTime
            setStatus(string.format("Extracting... %.1fs", elapsed), Color3.fromRGB(100, 180, 255), 0)
            task.wait(0.1)
        end
    end)

    -- Initialize reflection engine with API dump
    local apiDump = Reflection.loadAPIDump()
    if not apiDump then
        operationState.extractStartTime = nil
        warn("Can't load API. Try reopening Studio.")
        setStatus("Failed to load API", Color3.fromRGB(255, 100, 100), 5)
        isExtracting = false
        return
    end

    -- Debug: log what config contains
    print("[RbxSync Debug] config.project_dir:", config.project_dir or "nil")
    print("[RbxSync Debug] config.services type:", type(config.services), "length:", config.services and #config.services or "nil")

    -- Determine which services to extract (check for non-empty array since empty [] is truthy in Lua)
    local defaultServices = {
        "Workspace",
        "ReplicatedStorage",
        "ReplicatedFirst",
        "ServerScriptService",
        "ServerStorage",
        "StarterGui",
        "StarterPack",
        "StarterPlayer",
        "Lighting",
        "SoundService",
        "Teams",
        "Chat",
        "LocalizationService",
        "TestService",
        "MaterialService",
    }
    local servicesToExtract = (config.services and #config.services > 0) and config.services or defaultServices
    print("[RbxSync Debug] Using", #servicesToExtract, "services")

    -- Collect all instances to serialize
    local allInstances: {Instance} = {}

    print("[RbxSync Debug] Services to extract:", #servicesToExtract)
    for _, serviceName in servicesToExtract do
        local service = game:FindFirstChild(serviceName)
        if service then
            local count = #service:GetDescendants()
            print("[RbxSync Debug] Found service:", serviceName, "with", count, "descendants")
            table.insert(allInstances, service)
            for _, descendant in service:GetDescendants() do
                table.insert(allInstances, descendant)
            end
        else
            print("[RbxSync Debug] Service not found:", serviceName)
        end
    end
    print("[RbxSync Debug] Total instances collected:", #allInstances)

    -- Build disambiguated paths for all services to handle duplicate sibling names
    -- This needs to happen before serialization so paths are correctly computed
    for _, serviceName in servicesToExtract do
        local service = game:FindFirstChild(serviceName)
        if service then
            -- Build paths for the service itself and all descendants
            Serializer.buildDisambiguatedPaths(service)
        end
    end

    -- Calculate chunks
    local totalChunks = math.ceil(#allInstances / CHUNK_SIZE)
    local sessionId = HttpService:GenerateGUID(false)

    -- Extract and send terrain FIRST (before instance chunks) to avoid timeout
    local extractTerrain = config.extractTerrain == true
    print("[RbxSync Debug] extractTerrain flag:", config.extractTerrain, "resolved to:", extractTerrain)
    if extractTerrain then
        local hasWorkspace = false
        for _, serviceName in servicesToExtract do
            if serviceName == "Workspace" then
                hasWorkspace = true
                break
            end
        end

        if hasWorkspace then
            print("[RbxSync] Extracting terrain (this may take a moment)...")
            local terrainData = TerrainHandler.extractTerrain()
            if terrainData and #terrainData.chunks > 0 then
                print(string.format("[RbxSync] Sending terrain data: %d chunks", #terrainData.chunks))

                -- Send terrain metadata first (without chunks)
                local terrainMeta = {
                    chunkSize = terrainData.chunkSize,
                    resolution = terrainData.resolution,
                    region = terrainData.region,
                    properties = terrainData.properties,
                    totalChunks = #terrainData.chunks,
                }

                -- Send chunks in batches to stay under 1MB limit
                local CHUNKS_PER_BATCH = 3
                local allSuccess = true

                for i = 1, #terrainData.chunks, CHUNKS_PER_BATCH do
                    local batchChunks = {}
                    for j = i, math.min(i + CHUNKS_PER_BATCH - 1, #terrainData.chunks) do
                        table.insert(batchChunks, terrainData.chunks[j])
                    end

                    local batchData = {
                        chunkSize = terrainData.chunkSize,
                        resolution = terrainData.resolution,
                        region = terrainData.region,
                        properties = (i == 1) and terrainData.properties or nil, -- Only send properties with first batch
                        chunks = batchChunks,
                    }

                    local terrainSuccess, terrainResult = httpPost("/extract/terrain", {
                        project_dir = projectDir,
                        session_id = sessionId,
                        terrain = batchData,
                        batch_index = math.ceil(i / CHUNKS_PER_BATCH),
                        total_batches = math.ceil(#terrainData.chunks / CHUNKS_PER_BATCH),
                    })

                    if not terrainSuccess then
                        warn("[RbxSync] Failed to send terrain batch:", tostring(terrainResult))
                        allSuccess = false
                        break
                    end

                    task.wait(0.02) -- Minimal delay to yield
                end

                if allSuccess then
                    print("[RbxSync] Terrain data sent successfully")
                end
            else
                print("[RbxSync] No terrain data found")
            end
        end
    end

    -- First pass: serialize all chunks
    local serializedChunks = {}
    for chunkIndex = 0, totalChunks - 1 do
        local startIdx = chunkIndex * CHUNK_SIZE + 1
        local endIdx = math.min(startIdx + CHUNK_SIZE - 1, #allInstances)

        local chunkInstances = {}
        for i = startIdx, endIdx do
            local instance = allInstances[i]
            local serialized = Serializer.serializeInstance(instance, apiDump)
            if serialized then
                -- Store referenceId on instance for reliable lookup during sync
                if serialized.referenceId then
                    pcall(function()
                        instance:SetAttribute("_rbxsync_ref", serialized.referenceId)
                    end)
                end

                -- Special handling for UnionOperation: separate into component parts
                if instance:IsA("UnionOperation") or instance:IsA("IntersectOperation") then
                    local separated = CSGHandler.separate(plugin, instance :: BasePart)
                    if separated.success and (#separated.addParts > 0 or #separated.subtractParts > 0) then
                        -- Build CSG metadata
                        local operations = {}
                        local partIndex = 1

                        -- Serialize and add the component parts
                        for _, part in separated.addParts do
                            local partName = "CSGPart_" .. partIndex
                            local partPath = serialized.path .. "/_csg/" .. partName
                            local partSerialized = Serializer.serializeInstance(part, apiDump)
                            if partSerialized then
                                partSerialized.path = partPath
                                partSerialized.name = partName
                                table.insert(chunkInstances, partSerialized)
                                table.insert(operations, {
                                    type = "add",
                                    partRef = partName,
                                })
                            end
                            -- Clean up the separated part
                            part:Destroy()
                            partIndex += 1
                        end

                        -- Serialize subtracted parts (NegateOperations)
                        for _, part in separated.subtractParts do
                            local partName = "CSGNegate_" .. partIndex
                            local partPath = serialized.path .. "/_csg/" .. partName
                            local partSerialized = Serializer.serializeInstance(part, apiDump)
                            if partSerialized then
                                partSerialized.path = partPath
                                partSerialized.name = partName
                                table.insert(chunkInstances, partSerialized)
                                table.insert(operations, {
                                    type = "subtract",
                                    partRef = partName,
                                })
                            end
                            -- Clean up the separated part
                            part:Destroy()
                            partIndex += 1
                        end

                        -- Add CSG metadata to the union
                        if #operations > 0 then
                            serialized.csg = {
                                version = 1,
                                separable = true,
                                operations = operations,
                            }
                            print(string.format("[RbxSync] Separated union '%s' into %d parts", serialized.name, #operations))
                        end
                    elseif separated.error then
                        -- Mark as non-separable with error
                        serialized.csg = {
                            version = 1,
                            separable = false,
                            error = separated.error,
                        }
                        warn(string.format("[RbxSync] Could not separate union '%s': %s", serialized.name, separated.error))
                    end
                end

                table.insert(chunkInstances, serialized)
            end
        end
        serializedChunks[chunkIndex] = chunkInstances
    end

    -- Second pass: send chunks in parallel with concurrency limit
    local MAX_CONCURRENT = 5
    local activeRequests = 0
    local completedChunks = 0
    local failedChunk = false

    for chunkIndex = 0, totalChunks - 1 do
        -- Wait if at concurrency limit
        while activeRequests >= MAX_CONCURRENT do
            task.wait(0.01)
        end

        if failedChunk then
            break
        end

        activeRequests = activeRequests + 1
        task.spawn(function()
            local success = sendChunk(sessionId, chunkIndex, totalChunks, serializedChunks[chunkIndex], projectDir)
            if not success then
                failedChunk = true
            end
            completedChunks = completedChunks + 1
            activeRequests = activeRequests - 1
        end)
    end

    -- Wait for all chunks to complete
    while completedChunks < totalChunks and not failedChunk do
        task.wait(0.01)
    end

    if failedChunk then
        operationState.extractStartTime = nil
        warn("Extraction failed. Try again.")
        setStatus("Extraction failed", Color3.fromRGB(255, 100, 100), 5)
        isExtracting = false
        return
    end

    -- Finalize
    local finalizeSuccess, finalizeResult = httpPost("/extract/finalize", {
        project_dir = projectDir,
    })

    -- Stop timer
    operationState.extractStartTime = nil
    local elapsed = os.clock() - startTime

    if finalizeSuccess and finalizeResult.success then
        -- Save timing for future estimates and UI display
        lastExtractTime = elapsed
        lastInstanceCount = #allInstances
        operationState.lastExtractDuration = elapsed
        operationState.lastExtractCount = #allInstances
        local msg = string.format("✓ Extracted %d instances (%.1fs)", #allInstances, elapsed)
        print(msg)
        setStatus(msg, Color3.fromRGB(100, 200, 100), 10)
    else
        local errMsg = "Extraction failed"
        warn(errMsg .. ". " .. (finalizeResult and finalizeResult.error or "Try again."))
        setStatus(errMsg, Color3.fromRGB(255, 100, 100), 10)
    end

    isExtracting = false
end

-- List of services and built-in instances that cannot be replaced
local IMMUTABLE_CLASSES = {
    "Workspace", "ReplicatedStorage", "ReplicatedFirst",
    "ServerScriptService", "ServerStorage", "StarterGui",
    "StarterPack", "StarterPlayer", "Lighting", "SoundService",
    "Teams", "Chat", "LocalizationService", "TestService",
    "StarterCharacterScripts", "StarterPlayerScripts",
    "Terrain", "Camera",
}

local function isImmutableClass(className: string): boolean
    for _, name in IMMUTABLE_CLASSES do
        if className == name then
            return true
        end
    end
    return false
end

-- Sync: Create instance at path (silent)
-- Returns: success, error, skipped, action (what happened), reasons (what changed)
local function syncCreate(payload: any): {success: boolean, error: string?, skipped: boolean?, action: string?, reasons: {string}?}
    local path = payload.path
    local data = payload.data

    if not path or not data then
        return { success = false, error = "Missing path or data" }
    end

    -- Normalize path delimiters (Windows uses backslash)
    path = string.gsub(path, "\\", "/")
    payload.path = path
    if data.path then
        data.path = string.gsub(data.path, "\\", "/")
    end

    local className = data.className or ""

    -- Services and built-in instances: find by path/name, don't use referenceId
    -- They're singletons and should never be matched by referenceId
    if isImmutableClass(className) then
        local existingInstance = Sync.findInstanceAtPath(path)
        if existingInstance then
            -- Update properties only (can't replace)
            local needsUpdate, reasons = Sync.instanceNeedsUpdate(existingInstance, data)
            if needsUpdate then
                local _, replacedInstance = Sync.updateInstance(existingInstance, data)
                Sync.registerCreated(path, replacedInstance or existingInstance)
                return { success = true, action = "updated", reasons = reasons }
            end
            Sync.registerCreated(path, existingInstance)
            return { success = true, skipped = true }
        end
        -- For services, if not found at path, they probably exist under game
        local service = game:FindFirstChild(className)
        if service then
            Sync.registerCreated(path, service)
            return { success = true, skipped = true }
        end
        return { success = true, skipped = true } -- Services always "succeed" even if not found
    end

    -- FIRST: Try to find existing instance by referenceId
    local existingInstance: Instance? = nil
    if data.referenceId then
        existingInstance = Sync.findInstanceByRefId(data.referenceId)
        if existingInstance then
            -- Verify className matches to prevent wrong instance matches
            if existingInstance.ClassName == className then
                -- Check if this instance is actually valid (has parent)
                if not existingInstance.Parent then
                    existingInstance = nil
                else
                    local needsUpdate, reasons = Sync.instanceNeedsUpdate(existingInstance, data)
                    if needsUpdate then
                        local _, replacedInstance = Sync.updateInstance(existingInstance, data)
                        Sync.registerCreated(path, replacedInstance or existingInstance)
                        return { success = true, action = "updated", reasons = reasons }
                    end
                    Sync.registerCreated(path, existingInstance)
                    return { success = true, skipped = true }
                end
            else
                -- Wrong instance found by referenceId - ignore and continue
                existingInstance = nil
            end
        end
    end

    -- SECOND: Try to find by path
    existingInstance = Sync.findInstanceAtPath(path)
    if existingInstance then
        -- Check if this instance is actually valid (has parent)
        if not existingInstance.Parent then
            existingInstance = nil
        elseif existingInstance.ClassName == className then
            local needsUpdate, reasons = Sync.instanceNeedsUpdate(existingInstance, data)
            if needsUpdate then
                local _, replacedInstance = Sync.updateInstance(existingInstance, data)
                Sync.registerCreated(path, replacedInstance or existingInstance)
                return { success = true, action = "updated", reasons = reasons }
            end
            Sync.registerCreated(path, existingInstance)
            return { success = true, skipped = true }
        end
        -- Wrong className - will need to replace (handled below)
    end

    -- THIRD: Create new instance - find parent first
    local pathParts = string.split(path, "/")
    table.remove(pathParts) -- Remove the instance name itself
    local parentPath = table.concat(pathParts, "/")

    -- Special handling for CSG component parts (paths containing /_csg/)
    -- These are temporary parts used for union reconstruction, not parented to anything
    local isCSGPart = string.find(path, "/_csg/") ~= nil
    if isCSGPart then
        -- CSG parts are created without a parent - they're temporary for reconstruction
        local instance = Sync.createInstance(data, nil)
        if instance then
            Sync.registerCreated(path, instance)
            return { success = true, action = "created (CSG part)" }
        else
            return { success = false, error = "Failed to create CSG component part" }
        end
    end

    local parent: Instance? = nil
    if #parentPath > 0 then
        parent = Sync.findInstanceAtPath(parentPath)
        if not parent then
            return { success = false, error = "Parent not found: " .. parentPath }
        end
    end

    -- Create the instance
    local instance = Sync.createInstance(data, parent)
    if instance then
        Sync.registerCreated(path, instance)
        return { success = true, action = "created" }
    else
        -- Check if this was a CSG union deferred for reconstruction
        local className = data.className or ""
        if (className == "UnionOperation" or className == "IntersectOperation")
            and data.csg and data.csg.separable then
            -- Not a failure - deferred for CSG reconstruction
            return { success = true, skipped = true }
        end
        return { success = false, error = "Can't create instance. Parent may be locked." }
    end
end

-- Sync: Update instance at path (silent)
-- Returns: success, error, skipped, action, reasons
local function syncUpdate(payload: any): {success: boolean, error: string?, skipped: boolean?, action: string?, reasons: {string}?}
    local path = payload.path
    local data = payload.data

    if not path or not data then
        return { success = false, error = "Missing path or data" }
    end

    -- Normalize path delimiters (Windows uses backslash)
    path = string.gsub(path, "\\", "/")
    payload.path = path
    if data.path then
        data.path = string.gsub(data.path, "\\", "/")
    end

    local className = data.className or ""

    -- Handle immutable classes (services, built-in instances)
    if isImmutableClass(className) then
        local instance = Sync.findInstanceAtPath(path) or game:FindFirstChild(className)
        if instance then
            -- Only update properties, never replace
            local needsUpdate, reasons = Sync.instanceNeedsUpdate(instance, data)
            if needsUpdate then
                local _, replacedInstance = Sync.updateInstance(instance, data)
                Sync.registerCreated(path, replacedInstance or instance)
                return { success = true, action = "updated", reasons = reasons }
            end
            Sync.registerCreated(path, instance)
        end
        return { success = true, skipped = true }
    end

    local instance = Sync.findInstanceAtPath(path)
    if not instance then
        return syncCreate(payload)
    end

    -- If className changed and instance is not immutable, try to replace
    if data.className and instance.ClassName ~= data.className then

        -- Don't try to replace if the found instance is immutable
        if isImmutableClass(instance.ClassName) then
            return { success = true, skipped = true } -- Skip, can't replace immutable
        end

        -- Get parent before destroying
        local parent = instance.Parent

        -- Try to remove old instance
        local removeOk = pcall(function()
            instance.Parent = nil
        end)

        if not removeOk then
            -- Can't remove - just skip
            return { success = true, skipped = true }
        end

        -- Create new instance with correct className
        local newInstance = Sync.createInstance(data, parent)
        if newInstance then
            Sync.registerCreated(path, newInstance)
            return { success = true, action = "replaced", reasons = {"className:" .. (instance.ClassName) .. "->" .. data.className} }
        else
            -- Check if this was a CSG union deferred for reconstruction
            if (data.className == "UnionOperation" or data.className == "IntersectOperation")
                and data.csg and data.csg.separable then
                -- Not a failure - deferred for CSG reconstruction
                return { success = true, skipped = true }
            end
            return { success = false, error = "Can't create replacement instance" }
        end
    end

    -- Check if update is actually needed (differential sync)
    local needsUpdate, reasons = Sync.instanceNeedsUpdate(instance, data)
    if not needsUpdate then
        -- No changes needed - skip this update
        return { success = true, skipped = true }
    end

    local ok, replacedInstance = Sync.updateInstance(instance, data)
    Sync.registerCreated(path, replacedInstance or instance)
    if ok then
        return { success = true, action = "updated", reasons = reasons }
    else
        return { success = false, error = "Can't update instance" }
    end
end

-- Sync: Delete instance at path (silent)
local function syncDelete(payload: any): {success: boolean, error: string?, action: string?}
    local path = payload.path

    if not path then
        return { success = false, error = "Missing path" }
    end

    local ok = Sync.deleteInstance(path)
    if ok then
        return { success = true, action = "deleted" }
    else
        -- Instance not found - treat as skipped (already deleted)
        return { success = true, skipped = true }
    end
end

-- Sync: Apply batch of operations
local function syncBatch(payload: any): {success: boolean, results: {any}?, error: string?, applied: number?, skipped: number?}
    local operations = payload.operations
    local source = payload.source  -- "file_watcher" if from file watcher

    if not operations or type(operations) ~= "table" then
        return { success = false, error = "No operations to apply" }
    end

    -- Set waypoint BEFORE making changes for undo support
    local opSummary = #operations .. " operations"
    if #operations == 1 then
        local op = operations[1]
        opSummary = (op.type or "sync") .. " " .. (op.path or "")
    end
    ChangeHistoryService:SetWaypoint("Before RbxSync: " .. opSummary)

    -- Mark specific paths as file watcher sync to prevent echo back
    if source == "file_watcher" then
        local paths = {}
        for _, op in operations do
            if op.path then
                table.insert(paths, op.path)
            end
        end
        ChangeTracker.setLastFileWatcherSync(paths)
    end

    -- Sort operations: creates first (by path depth), then updates, then deletes (reverse depth)
    -- This ensures parents are created before children
    table.sort(operations, function(a, b)
        local aType = a.type or "update"
        local bType = b.type or "update"
        local aPath = a.path or ""
        local bPath = b.path or ""

        -- Count path segments (depth)
        local aDepth = select(2, string.gsub(aPath, "/", "")) + 1
        local bDepth = select(2, string.gsub(bPath, "/", "")) + 1

        -- Creates first, then updates, then deletes
        local typePriority = { create = 1, update = 2, delete = 3 }
        local aPriority = typePriority[aType] or 2
        local bPriority = typePriority[bType] or 2

        if aPriority ~= bPriority then
            return aPriority < bPriority
        end

        -- For creates/updates: shallow paths first (parents before children)
        -- For deletes: deep paths first (children before parents)
        if aType == "delete" then
            if aDepth ~= bDepth then
                return aDepth > bDepth
            end
        else
            if aDepth ~= bDepth then
                return aDepth < bDepth
            end
        end

        -- For same depth, sort by path to ensure parent paths come before child paths
        -- This handles cases like "A/B" before "A/B/C" at same calculated depth
        return aPath < bPath
    end)

    -- Debug: verify sort order (first 20 and last 20 operations by depth)
    if #operations > 0 then
        local firstDepth = select(2, string.gsub(operations[1].path or "", "/", "")) + 1
        local lastDepth = select(2, string.gsub(operations[#operations].path or "", "/", "")) + 1
        print(string.format("[RbxSync Debug] Sort check: first depth=%d, last depth=%d, total=%d", firstDepth, lastDepth, #operations))

        -- Check for any out-of-order items
        local prevDepth = 0
        local outOfOrder = 0
        for i, op in ipairs(operations) do
            local depth = select(2, string.gsub(op.path or "", "/", "")) + 1
            if depth < prevDepth then
                outOfOrder += 1
                if outOfOrder <= 3 then
                    print(string.format("[RbxSync Debug] Out of order at %d: %s (depth %d) after depth %d", i, op.path or "nil", depth, prevDepth))
                end
            end
            prevDepth = depth
        end
        if outOfOrder > 0 then
            print(string.format("[RbxSync Debug] Total out of order: %d", outOfOrder))
        end
    end

    -- Clear cache before sync to handle undo/redo scenarios
    Sync.clearRecentlyCreated()
    Sync.clearPendingReferences()

    isSyncing = true
    setStatus("Checking " .. #operations .. " instances...", Color3.fromRGB(100, 180, 255), 0)
    local results = {}
    local appliedCount = 0  -- Actually modified
    local skippedCount = 0  -- No changes needed
    local failCount = 0
    local changes: {{path: string, action: string, reasons: {string}?}} = {}  -- Track what changed

    -- Wrap in pcall to ensure isSyncing is always reset even on error
    local syncOk, syncErr = pcall(function()
        for i, op in ipairs(operations) do
            local opType = op.type
            local result

            if opType == "create" then
                result = syncCreate(op)
            elseif opType == "update" then
                result = syncUpdate(op)
            elseif opType == "delete" then
                result = syncDelete(op)
            else
                result = { success = false, error = "Unknown operation" }
            end

            table.insert(results, result)

            if result.success then
                if result.skipped then
                    skippedCount += 1
                else
                    appliedCount += 1
                    -- Track what changed for debugging output
                    if result.action then
                        table.insert(changes, {
                            path = op.path or "unknown",
                            action = result.action,
                            reasons = result.reasons
                        })
                    end
                end
            else
                failCount += 1
                -- Log failed operations with path and error
                warn(string.format("  Failed: %s - %s", op.path or "unknown", result.error or "unknown error"))
            end

            -- Yield occasionally to prevent freezing
            if i % 100 == 0 then
                task.wait()
            end
        end

        -- Second pass: resolve reference properties (Part0, Part1, etc.)
        local resolved = Sync.resolvePendingReferences()
        if resolved > 0 then
            print(string.format("[RbxSync] Resolved %d reference properties", resolved))
        end

        -- Third pass: reconstruct CSG operations (unions from component parts)
        local csgReconstructed = Sync.reconstructPendingCSG()
        if csgReconstructed > 0 then
            print(string.format("[RbxSync] Reconstructed %d CSG operations", csgReconstructed))
        end

        -- Fourth pass: apply material overrides (must happen after all MaterialVariants are created)
        local materialsApplied = Sync.applyPendingMaterialOverrides()
        if materialsApplied > 0 then
            print(string.format("[RbxSync] Applied %d material overrides", materialsApplied))
        end
    end)

    -- ALWAYS reset isSyncing, even on error
    isSyncing = false

    -- If sync errored out, report it
    if not syncOk then
        warn("[RbxSync] Sync error: " .. tostring(syncErr))
        setStatus("Sync error occurred", Color3.fromRGB(255, 100, 100), 10)
        return { success = false, results = results, applied = appliedCount, skipped = skippedCount }
    end

    -- Print detailed changes (only things that actually changed)
    if #changes > 0 then
        print("[RbxSync] Changes applied:")
        for _, change in changes do
            local reasonsStr = ""
            if change.reasons and #change.reasons > 0 then
                reasonsStr = " [" .. table.concat(change.reasons, ", ") .. "]"
            end
            print(string.format("  %s: %s%s", change.action, change.path, reasonsStr))
        end
    end

    -- Summary - show actual changes vs unchanged
    if failCount > 0 then
        local msg = string.format("Applied %d, skipped %d unchanged, %d failed", appliedCount, skippedCount, failCount)
        print(msg .. " (see warnings above)")
        setStatus(msg, Color3.fromRGB(255, 200, 100), 10)
    elseif appliedCount == 0 then
        local msg = "Already in sync"
        print(msg .. " (" .. skippedCount .. " instances checked)")
        setStatus(msg, Color3.fromRGB(100, 200, 100), 5)
    else
        local msg = string.format("Applied %d changes", appliedCount)
        if skippedCount > 0 then
            msg = msg .. string.format(" (%d unchanged)", skippedCount)
        end
        print(msg)
        setStatus(msg, Color3.fromRGB(100, 200, 100), 10)
    end

    -- Set waypoint AFTER changes for undo support
    if appliedCount > 0 then
        ChangeHistoryService:SetWaypoint("RbxSync: " .. opSummary)
    end

    return { success = failCount == 0, results = results, applied = appliedCount, skipped = skippedCount }
end

-- Handle incoming commands from server (silent dispatch)
local function handleCommand(command: string, payload: any)
    if command == "extract:start" then
        extractGame(payload or {})
    elseif command == "sync:create" then
        return syncCreate(payload or {})
    elseif command == "sync:update" then
        return syncUpdate(payload or {})
    elseif command == "sync:delete" then
        return syncDelete(payload or {})
    elseif command == "sync:batch" then
        return syncBatch(payload or {})
    elseif command == "ping" then
        return { success = true }
    elseif command == "test:start" then
        return TestRunner.startCapture()
    elseif command == "test:stop" then
        return TestRunner.stopCapture()
    elseif command == "test:output" then
        return TestRunner.getOutput()
    elseif command == "test:clear" then
        return TestRunner.clearOutput()
    elseif command == "test:errors" then
        return { success = true, errors = TestRunner.getErrors() }
    elseif command == "test:history" then
        return TestRunner.getLogHistory()
    elseif command == "test:run" then
        return TestRunner.startBackgroundTest(payload)
    elseif command == "test:status" then
        return TestRunner.getTestStatus()
    elseif command == "test:finish" then
        return TestRunner.stopBackgroundTest()
    elseif command == "test:isRunning" then
        return { success = true, running = TestRunner.isRunning() }
    elseif command == "test:forceStop" then
        return TestRunner.forceStopTest()
    elseif command == "debug:start" then
        -- Start playtest
        local RunService = game:GetService("RunService")
        if RunService:IsRunning() then
            return { success = false, error = "Playtest already running" }
        end
        -- Note: Direct playtest control is not available via plugin API
        return {
            success = false,
            error = "Starting playtests programmatically is not supported by Roblox Studio. Please press F5 (Run), F6 (Play), or F7 (Server) in Studio."
        }
    elseif command == "debug:stop" then
        -- Stop playtest
        local RunService = game:GetService("RunService")
        if not RunService:IsRunning() then
            return { success = false, error = "No playtest running" }
        end
        -- Note: Direct playtest control is not available via plugin API
        return {
            success = false,
            error = "Stopping playtests programmatically is not supported by Roblox Studio. Please press Shift+F5 or the Stop button in Studio."
        }
    elseif command == "debug:status" then
        -- Get playtest status
        local RunService = game:GetService("RunService")
        local isRunning = RunService:IsRunning()
        local isEdit = RunService:IsEdit()
        local isClient = RunService:IsClient()
        local isServer = RunService:IsServer()
        local mode = "edit"
        if isRunning then
            if isClient and isServer then
                mode = "play"  -- Play Solo (both client and server)
            elseif isServer then
                mode = "server"
            elseif isClient then
                mode = "client"
            else
                mode = "run"
            end
        end
        return {
            success = true,
            data = {
                running = isRunning,
                mode = mode,
                isEdit = isEdit,
                isClient = isClient,
                isServer = isServer
            }
        }
    elseif command == "run:code" then
        -- Execute arbitrary Luau code and return output
        local code = payload.code
        if not code or code == "" then
            return { success = false, error = "No code provided" }
        end
        -- Capture print output
        local output = {}
        local oldPrint = print
        local function capturePrint(...)
            local args = {...}
            local strs = {}
            for i, v in ipairs(args) do
                strs[i] = tostring(v)
            end
            table.insert(output, table.concat(strs, "\t"))
            oldPrint(...)
        end
        -- Execute code dynamically using standard plugin APIs
        local execFn = executeCode or loadstring
        if not execFn then
            return { success = false, error = "Code execution not available. Enable LoadStringEnabled in Studio settings.", output = "" }
        end
        local fn, loadErr = execFn(code)
        if not fn then
            return { success = false, error = "Syntax error: " .. tostring(loadErr), output = "" }
        end
        -- Set up environment for print capture
        local getFn = getEnvironment or getfenv
        local setFn = setEnvironment or setfenv
        if getFn and setFn then
            local env = setmetatable({ print = capturePrint }, { __index = getFn() })
            setFn(fn, env)
        end
        local ok, runErr = pcall(fn)
        if not ok then
            return { success = false, error = "Runtime error: " .. tostring(runErr), output = table.concat(output, "\n") }
        end
        return { success = true, output = table.concat(output, "\n") }
    elseif command == "insert:model" then
        -- Insert a model from the Roblox marketplace
        local query = payload.query
        if not query or query == "" then
            return { success = false, error = "No search query provided" }
        end

        -- Use InsertService to search and load the model
        local InsertService = game:GetService("InsertService")
        local MarketplaceService = game:GetService("MarketplaceService")

        -- Search for free models matching the query
        local searchSuccess, searchResult = pcall(function()
            return InsertService:GetFreeModels(query)
        end)

        if not searchSuccess then
            return { success = false, error = "Search failed: " .. tostring(searchResult) }
        end

        if not searchResult or #searchResult == 0 then
            return { success = false, error = "No models found for query: " .. query }
        end

        -- Get the first result
        local firstResult = searchResult[1]
        local assetId = firstResult.AssetId
        local assetName = firstResult.Name or "Unknown"

        -- Load and insert the model using standard InsertService API
        local loadSuccess, model = pcall(function()
            return InsertService:LoadAsset(assetId)
        end)

        if not loadSuccess then
            return { success = false, error = "Failed to load model: " .. tostring(model) }
        end

        -- Move contents to workspace
        if model then
            for _, child in ipairs(model:GetChildren()) do
                child.Parent = workspace
            end
            model:Destroy()

            return {
                success = true,
                data = {
                    modelName = assetName,
                    assetId = assetId
                }
            }
        else
            return { success = false, error = "Model loaded but was empty" }
        end
    elseif command == "terrain:sync" then
        -- Sync terrain from file data
        local terrainData = payload and payload.terrain
        if not terrainData then
            return { success = false, error = "No terrain data provided" }
        end

        local clearFirst = payload.clear ~= false -- Default to clearing first
        if clearFirst then
            TerrainHandler.clearTerrain()
        end

        local ok, chunksApplied = TerrainHandler.applyTerrain(terrainData)
        if ok then
            return { success = true, chunksApplied = chunksApplied }
        else
            return { success = false, error = "Failed to apply terrain" }
        end
    elseif command == "terrain:clear" then
        -- Clear all terrain
        local ok = TerrainHandler.clearTerrain()
        return { success = ok }
    elseif command == "studio:paths" then
        -- Get all instance paths in tracked services (for diff comparison)
        local paths = {}
        local trackedServices = {
            "Workspace", "ReplicatedStorage", "ReplicatedFirst",
            "ServerScriptService", "ServerStorage", "StarterGui",
            "StarterPack", "StarterPlayer", "Lighting", "SoundService",
            "Teams", "Chat", "LocalizationService", "TestService"
        }

        for _, serviceName in trackedServices do
            local service = game:FindFirstChild(serviceName)
            if service then
                -- Add service itself
                table.insert(paths, {
                    path = serviceName,
                    className = service.ClassName,
                    name = service.Name
                })

                -- Add all descendants
                for _, desc in service:GetDescendants() do
                    -- Build path by walking up parent chain
                    local parts = {}
                    local current = desc
                    while current and current ~= game do
                        table.insert(parts, 1, current.Name)
                        current = current.Parent
                    end

                    if #parts > 0 then
                        local path = table.concat(parts, "/")
                        table.insert(paths, {
                            path = path,
                            className = desc.ClassName,
                            name = desc.Name
                        })
                    end
                end
            end
        end

        return { success = true, paths = paths, count = #paths }

    -- Bot controller commands (AI-powered automated testing)
    elseif command == "bot:command" then
        -- Execute a bot command (movement, action, UI, observation)
        if not payload or not payload.type or not payload.command then
            return { success = false, error = "Missing command type or command name" }
        end
        return BotController.executeCommand(payload)

    elseif command == "bot:state" then
        -- Get current game state observation
        return { success = true, state = BotController.getState() }

    elseif command == "bot:move" then
        -- Direct movement command
        if payload.position then
            return BotController.moveTo(payload.position)
        elseif payload.object or payload.objectName then
            return BotController.moveToObject(payload.object or payload.objectName)
        else
            return { success = false, error = "Missing position or object name" }
        end

    elseif command == "bot:stop" then
        -- Stop current movement
        BotController.stopMovement()
        return { success = true }

    elseif command == "bot:action" then
        -- Execute an action command
        local actionType = payload.action or payload.type
        if actionType == "equip" then
            return BotController.equipTool(payload.name or payload.toolName)
        elseif actionType == "unequip" then
            return BotController.unequipTool()
        elseif actionType == "activate" then
            return BotController.activateTool()
        elseif actionType == "deactivate" then
            return BotController.deactivateTool()
        elseif actionType == "interact" then
            return BotController.interact(payload.name or payload.objectName)
        elseif actionType == "jump" then
            return BotController.jump()
        else
            return { success = false, error = "Unknown action type: " .. tostring(actionType) }
        end

    elseif command == "bot:ui" then
        -- Execute a UI command
        local uiAction = payload.action or payload.type
        if uiAction == "click" then
            return BotController.clickButton(payload.path or payload.buttonPath)
        elseif uiAction == "read" then
            return BotController.readText(payload.path or payload.labelPath)
        elseif uiAction == "fill" then
            return BotController.fillTextBox(payload.path or payload.textBoxPath, payload.text)
        elseif uiAction == "visible" then
            return { success = true, ui = BotController.getVisibleUI() }
        else
            return { success = false, error = "Unknown UI action: " .. tostring(uiAction) }
        end

    elseif command == "bot:observe" then
        -- Observation commands
        local observeType = payload.type or "state"
        if observeType == "state" then
            return { success = true, state = BotController.getState() }
        elseif observeType == "nearby" then
            return { success = true, objects = BotController.getNearbyObjects(payload.radius) }
        elseif observeType == "npcs" then
            return { success = true, npcs = BotController.getNearbyNPCs(payload.radius) }
        elseif observeType == "inventory" then
            return { success = true, inventory = BotController.getInventory() }
        elseif observeType == "find" then
            return { success = true, results = BotController.findObjects(payload.query or payload.name) }
        else
            return { success = false, error = "Unknown observe type: " .. tostring(observeType) }
        end

    elseif command == "bot:config" then
        -- Configure bot settings
        if payload.get then
            return { success = true, config = BotController.getConfig() }
        else
            BotController.configure(payload)
            return { success = true, config = BotController.getConfig() }
        end

    else
        return { success = false, error = "Unknown command" }
    end
end

-- Main polling loop (silent)
local function pollLoop()
    while true do
        if isConnected then
            -- Include projectDir for multi-workspace routing
            local projectDir = Config.getProjectDir()
            local endpoint = "/rbxsync/request"
            if projectDir ~= "" then
                endpoint = endpoint .. "?projectDir=" .. HttpService:UrlEncode(projectDir)
            end
            local success, response = httpGet(endpoint)

            -- Detect disconnection from failed request
            if not success then
                isConnected = false
                print("Disconnected")
                updateConnectionUI()
            elseif response and response.command then
                print("[RbxSync Debug] Received command: " .. tostring(response.command))
                if response.command == "extract:start" then
                    print("[RbxSync Debug] Extract payload:", HttpService:JSONEncode(response.payload or {}))
                end

                -- Check if this is a sync command and if files→studio sync is disabled or extraction is in progress
                local command = response.command
                local isSyncCommand = command == "sync:batch" or command == "sync:create" or command == "sync:update" or command == "sync:delete"

                if isSyncCommand and not syncSettings.filesToStudio then
                    -- Skip sync commands when files→studio is disabled
                    print("[RbxSync] Skipping sync command (files→studio disabled)")
                    -- Still send response to avoid timeout
                    if response.id then
                        httpPost("/rbxsync/response", {
                            id = response.id,
                            success = true,
                            data = { skipped = true, reason = "files_to_studio_disabled" },
                        })
                    end
                elseif isSyncCommand and isExtracting then
                    -- Skip sync commands while extraction is in progress to avoid conflicts
                    print("[RbxSync] Skipping sync command (extraction in progress)")
                    if response.id then
                        httpPost("/rbxsync/response", {
                            id = response.id,
                            success = true,
                            data = { skipped = true, reason = "extraction_in_progress" },
                        })
                    end
                else
                    task.spawn(function()
                        -- Create descriptive checkpoint name for undo
                        local checkpointName = "RbxSync"
                        local payload = response.payload

                        if command == "sync:batch" and payload and payload.operations then
                        local ops = payload.operations
                        local opCount = #ops
                        if opCount == 1 then
                            local op = ops[1]
                            checkpointName = string.format("RbxSync: %s %s", op.type or "sync", op.path or "")
                        elseif opCount > 1 then
                            -- Summarize batch operations
                            local types = {}
                            for _, op in ops do
                                types[op.type or "sync"] = (types[op.type or "sync"] or 0) + 1
                            end
                            local parts = {}
                            for t, c in types do
                                table.insert(parts, c .. " " .. t)
                            end
                            checkpointName = "RbxSync: " .. table.concat(parts, ", ")
                        end
                    elseif command == "sync:delete" and payload then
                        checkpointName = "RbxSync: delete " .. (payload.path or "")
                    elseif command == "sync:create" and payload then
                        checkpointName = "RbxSync: create " .. (payload.path or "")
                    elseif command == "sync:update" and payload then
                        checkpointName = "RbxSync: update " .. (payload.path or "")
                    end

                    -- Set waypoint BEFORE making changes
                    ChangeHistoryService:SetWaypoint("Before: " .. checkpointName)

                    local ok, result = pcall(function()
                        return handleCommand(response.command, response.payload)
                    end)

                        -- Set waypoint AFTER making changes
                        ChangeHistoryService:SetWaypoint(checkpointName)
                        print("[RbxSync] Waypoint set: " .. checkpointName)

                        -- Send response
                        if response.id then
                            local responseData = {
                                id = response.id,
                                success = ok and (result == nil or result.success ~= false),
                                data = ok and result or {},  -- Never send nil, use empty table
                                error = (not ok and tostring(result)) or (result and result.error) or nil,
                            }
                            print("[RbxSync Debug] Sending response for:", response.command, "id:", response.id)
                            local postOk, postErr = httpPost("/rbxsync/response", responseData)
                            if not postOk then
                                warn("[RbxSync] Failed to send response:", postErr)
                            else
                                print("[RbxSync Debug] Response sent successfully")
                            end
                        end
                    end)
                end
            end
        end

        task.wait(POLL_INTERVAL)
    end
end

-- Register with server (sends place info for multi-workspace support)
local function registerWithServer()
    local projectDir = Config.getProjectDir()
    if projectDir == "" then
        return false
    end

    local placeName = getPlaceName()
    local success, result = httpPost("/rbxsync/register", {
        place_id = game.PlaceId,
        place_name = placeName,
        project_dir = projectDir,
        session_id = SESSION_ID,  -- Unique ID for this Studio instance
    })

    if success and not hasLoggedLink then
        hasLoggedLink = true
        print(string.format("Linked to %s (%s)", projectDir, placeName))
    end

    return success
end

-- Unregister from server (called when Studio closes)
local function unregisterWithServer()
    local projectDir = Config.getProjectDir()
    if projectDir == "" then
        return false
    end

    local placeName = getPlaceName()
    local success, _ = httpPost("/rbxsync/unregister", {
        place_id = game.PlaceId,
        place_name = placeName,
        project_dir = projectDir,
        session_id = SESSION_ID,  -- Unique ID for this Studio instance
    })

    if success then
        print("[RbxSync] Unregistered from server")
    end

    return success
end

-- Unlink from workspace (keeps Studio visible but clears project_dir)
local function unlinkFromServer()
    local placeName = getPlaceName()
    -- Use register with empty project_dir to stay visible but unlinked
    local success, _ = httpPost("/rbxsync/register", {
        place_id = game.PlaceId,
        place_name = placeName,
        project_dir = "",  -- Empty project_dir = unlinked but still registered
        session_id = SESSION_ID,
    })

    if success then
        print("[RbxSync] Unlinked from workspace")
    end

    return success
end

-- Check server connection
local function checkConnection()
    local success, _ = httpGet("/health")

    if success ~= isConnected then
        isConnected = success
        if isConnected then
            print("Connected")
            -- Register with server for multi-workspace routing
            registerWithServer()
        else
            print("Disconnected")
        end
    end
end

-- ============================================================================
-- UI Creation (Minimal Design)
-- ============================================================================

-- Create toolbar
local toolbar = plugin:CreateToolbar("RbxSync")
local panelButton = toolbar:CreateButton(
    "RbxSync",
    "Open RbxSync panel",
    "rbxassetid://6031071053"
)

-- Create dock widget
local widgetInfo = DockWidgetPluginGuiInfo.new(
    Enum.InitialDockState.Right,
    false,
    false,
    280,
    400,
    240,
    250
)

local widget = plugin:CreateDockWidgetPluginGui("RbxSyncPanel", widgetInfo)
widget.Title = "RbxSync"

-- ============================================================================
-- DESIGN TOKENS (Roblox Studio Default Style)
-- ============================================================================

local Colors = {
    -- Matches Roblox Studio's dark theme
    background = Color3.fromRGB(46, 46, 46),
    surface = Color3.fromRGB(53, 53, 53),
    surfaceHover = Color3.fromRGB(66, 66, 66),
    border = Color3.fromRGB(34, 34, 34),
    -- Roblox blue accent
    accent = Color3.fromRGB(0, 162, 255),
    accentHover = Color3.fromRGB(50, 181, 255),
    success = Color3.fromRGB(2, 183, 87),
    warning = Color3.fromRGB(245, 166, 35),
    error = Color3.fromRGB(255, 68, 68),
    textPrimary = Color3.fromRGB(204, 204, 204),
    textSecondary = Color3.fromRGB(153, 153, 153),
    textMuted = Color3.fromRGB(102, 102, 102),
}

-- Change tracking for button badges
local changeTracking = {
    studioChanges = 0,  -- Files changed in Studio (needs extract)
    localChanges = 0,   -- Files changed locally (needs sync)
}

-- Forward declaration for button update function
local updateButtonBadges: () -> ()

-- ============================================================================
-- BUTTON STATE MANAGEMENT
-- ============================================================================

-- Sync button state enum
local SyncButtonState = {
    IDLE = "idle",
    SYNCING = "syncing",
    DISCONNECTED = "disconnected",
    SUCCESS = "success",
}

local currentSyncState = SyncButtonState.DISCONNECTED
local syncPulseConnection: RBXScriptConnection? = nil

-- Forward declarations for button state functions (defined after buttons exist)
local updateSyncButtonState: (state: string) -> ()
local updateExtractButtonState: (connected: boolean) -> ()

-- ============================================================================
-- MAIN FRAME
-- ============================================================================

local mainFrame = Instance.new("Frame")
mainFrame.Size = UDim2.new(1, 0, 1, 0)
mainFrame.BackgroundColor3 = Colors.background
mainFrame.BorderSizePixel = 0
mainFrame.Parent = widget

local layout = Instance.new("UIListLayout")
layout.SortOrder = Enum.SortOrder.LayoutOrder
layout.Padding = UDim.new(0, 0)
layout.Parent = mainFrame

-- ============================================================================
-- HEADER SECTION
-- ============================================================================

local headerFrame = Instance.new("Frame")
headerFrame.Name = "Header"
headerFrame.Size = UDim2.new(1, 0, 0, 52)
headerFrame.BackgroundColor3 = Colors.surface
headerFrame.BorderSizePixel = 0
headerFrame.LayoutOrder = 0
headerFrame.Parent = mainFrame

local accentLine = Instance.new("Frame")
accentLine.Size = UDim2.new(1, 0, 0, 2)
accentLine.BackgroundColor3 = Colors.accent
accentLine.BorderSizePixel = 0
accentLine.Parent = headerFrame

local headerPadding = Instance.new("UIPadding")
headerPadding.PaddingTop = UDim.new(0, 14)
headerPadding.PaddingLeft = UDim.new(0, 16)
headerPadding.Parent = headerFrame

local titleLabel = Instance.new("TextLabel")
titleLabel.Size = UDim2.new(1, 0, 0, 18)
titleLabel.Position = UDim2.new(0, 0, 0, 2)
titleLabel.BackgroundTransparency = 1
titleLabel.Text = "RBXSYNC"
titleLabel.TextColor3 = Colors.textPrimary
titleLabel.TextSize = 15
titleLabel.Font = Enum.Font.GothamBold
titleLabel.TextXAlignment = Enum.TextXAlignment.Left
titleLabel.Parent = headerFrame

local subtitleLabel = Instance.new("TextLabel")
subtitleLabel.Size = UDim2.new(1, 0, 0, 14)
subtitleLabel.Position = UDim2.new(0, 0, 0, 22)
subtitleLabel.BackgroundTransparency = 1
subtitleLabel.Text = "Studio ↔ VS Code"
subtitleLabel.TextColor3 = Colors.textMuted
subtitleLabel.TextSize = 10
subtitleLabel.Font = Enum.Font.Gotham
subtitleLabel.TextXAlignment = Enum.TextXAlignment.Left
subtitleLabel.Parent = headerFrame

-- ============================================================================
-- Connection Status + Button
-- ============================================================================

local connectionSection = Instance.new("Frame")
connectionSection.Size = UDim2.new(1, 0, 0, 56)
connectionSection.BackgroundTransparency = 1
connectionSection.LayoutOrder = 1
connectionSection.Parent = mainFrame

local connectionPadding = Instance.new("UIPadding")
connectionPadding.PaddingTop = UDim.new(0, 12)
connectionPadding.PaddingLeft = UDim.new(0, 16)
connectionPadding.PaddingRight = UDim.new(0, 16)
connectionPadding.Parent = connectionSection

local connectionFrame = Instance.new("Frame")
connectionFrame.Size = UDim2.new(1, 0, 0, 36)
connectionFrame.BackgroundColor3 = Colors.surface
connectionFrame.BorderSizePixel = 0
connectionFrame.Parent = connectionSection

local connectionCorner = Instance.new("UICorner")
connectionCorner.CornerRadius = UDim.new(0, 6)
connectionCorner.Parent = connectionFrame

local connectionStroke = Instance.new("UIStroke")
connectionStroke.Color = Colors.textMuted
connectionStroke.Thickness = 1
connectionStroke.Transparency = 0.8
connectionStroke.Parent = connectionFrame

local connectionDot = Instance.new("Frame")
connectionDot.Size = UDim2.new(0, 8, 0, 8)
connectionDot.Position = UDim2.new(0, 12, 0.5, -4)
connectionDot.BackgroundColor3 = Colors.textMuted
connectionDot.BorderSizePixel = 0
connectionDot.Parent = connectionFrame

local dotCorner = Instance.new("UICorner")
dotCorner.CornerRadius = UDim.new(1, 0)
dotCorner.Parent = connectionDot

-- Glow effect behind dot
local dotGlow = Instance.new("Frame")
dotGlow.Size = UDim2.new(0, 16, 0, 16)
dotGlow.Position = UDim2.new(0.5, -8, 0.5, -8)
dotGlow.BackgroundColor3 = Colors.success
dotGlow.BackgroundTransparency = 0.7
dotGlow.BorderSizePixel = 0
dotGlow.ZIndex = 0
dotGlow.Visible = false
dotGlow.Parent = connectionDot

local dotGlowCorner = Instance.new("UICorner")
dotGlowCorner.CornerRadius = UDim.new(1, 0)
dotGlowCorner.Parent = dotGlow

local connectionLabel = Instance.new("TextLabel")
connectionLabel.Size = UDim2.new(1, -100, 1, 0)
connectionLabel.Position = UDim2.new(0, 28, 0, 0)
connectionLabel.BackgroundTransparency = 1
connectionLabel.Text = "Disconnected"
connectionLabel.TextColor3 = Colors.textSecondary
connectionLabel.TextSize = 12
connectionLabel.Font = Enum.Font.GothamMedium
connectionLabel.TextXAlignment = Enum.TextXAlignment.Left
connectionLabel.Parent = connectionFrame

local connectButton = Instance.new("TextButton")
connectButton.Size = UDim2.new(0, 76, 0, 26)
connectButton.Position = UDim2.new(1, -81, 0.5, -13)
connectButton.BackgroundColor3 = Colors.accent
connectButton.BorderSizePixel = 0
connectButton.Text = "Connect"
connectButton.TextColor3 = Colors.background
connectButton.TextSize = 11
connectButton.Font = Enum.Font.GothamBold
connectButton.AutoButtonColor = true
connectButton.Parent = connectionFrame

local connectBtnCorner = Instance.new("UICorner")
connectBtnCorner.CornerRadius = UDim.new(0, 4)
connectBtnCorner.Parent = connectButton

updateConnectionUI = function()
    if isConnected then
        connectionLabel.Text = "Connected"
        connectionLabel.TextColor3 = Colors.success
        connectionDot.BackgroundColor3 = Colors.success
        dotGlow.Visible = true
        connectButton.Text = "Disconnect"
        connectButton.BackgroundColor3 = Colors.surface
        connectButton.TextColor3 = Colors.textPrimary
        -- Update button states
        updateSyncButtonState(SyncButtonState.IDLE)
        updateExtractButtonState(true)
    else
        connectionLabel.Text = "Disconnected"
        connectionLabel.TextColor3 = Colors.textSecondary
        connectionDot.BackgroundColor3 = Colors.textMuted
        dotGlow.Visible = false
        connectButton.Text = "Connect"
        connectButton.BackgroundColor3 = Colors.accent
        connectButton.TextColor3 = Colors.background
        -- Update button states
        updateSyncButtonState(SyncButtonState.DISCONNECTED)
        updateExtractButtonState(false)
    end
end

connectButton.MouseButton1Click:Connect(function()
    if isConnected then
        -- User clicked Disconnect
        unlinkFromServer()  -- Clear project_dir but stay visible in VS Code
        isConnected = false
        userDisconnected = true  -- Prevent auto-reconnect
        hasLoggedLink = false  -- Reset so we log again on reconnect
        ChangeTracker.stop()  -- Stop change tracking when disconnected
        stopConsoleCapture()  -- Stop console capture when disconnected
        print("Disconnected by user")
    else
        -- User clicked Connect
        userDisconnected = false  -- Allow reconnection
        local success, _ = httpGet("/health")
        if success then
            isConnected = true
            registerWithServer()
            -- Start change tracker for auto-extract (if enabled)
            if syncSettings.studioToFiles then
                ChangeTracker.start()
            end
            -- Start console capture for E2E testing
            startConsoleCapture()
        else
            isConnected = false
            warn("Could not connect to server")
        end
    end
    updateConnectionUI()
end)

-- ============================================================================
-- Project Path with Workspace Dropdown
-- ============================================================================

local projectSection = Instance.new("Frame")
projectSection.Size = UDim2.new(1, 0, 0, 72)
projectSection.BackgroundTransparency = 1
projectSection.LayoutOrder = 2
projectSection.Parent = mainFrame

local projectPadding = Instance.new("UIPadding")
projectPadding.PaddingTop = UDim.new(0, 0)
projectPadding.PaddingLeft = UDim.new(0, 16)
projectPadding.PaddingRight = UDim.new(0, 16)
projectPadding.Parent = projectSection

local projectLabel = Instance.new("TextLabel")
projectLabel.Size = UDim2.new(1, 0, 0, 16)
projectLabel.BackgroundTransparency = 1
projectLabel.Text = "PROJECT"
projectLabel.TextColor3 = Colors.textMuted
projectLabel.TextSize = 10
projectLabel.Font = Enum.Font.GothamBold
projectLabel.TextXAlignment = Enum.TextXAlignment.Left
projectLabel.Parent = projectSection

local projectPathFrame = Instance.new("Frame")
projectPathFrame.Size = UDim2.new(1, 0, 0, 36)
projectPathFrame.Position = UDim2.new(0, 0, 0, 20)
projectPathFrame.BackgroundColor3 = Colors.surface
projectPathFrame.BorderSizePixel = 0
projectPathFrame.Parent = projectSection

local projectPathCorner = Instance.new("UICorner")
projectPathCorner.CornerRadius = UDim.new(0, 6)
projectPathCorner.Parent = projectPathFrame

local projectPathStroke = Instance.new("UIStroke")
projectPathStroke.Color = Colors.textMuted
projectPathStroke.Thickness = 1
projectPathStroke.Transparency = 0.8
projectPathStroke.Parent = projectPathFrame

local projectDirInput = Instance.new("TextBox")
projectDirInput.Size = UDim2.new(1, -40, 1, 0)
projectDirInput.BackgroundTransparency = 1
projectDirInput.Text = Config.getProjectDir()
projectDirInput.PlaceholderText = "~/path/to/project"
projectDirInput.PlaceholderColor3 = Colors.textMuted
projectDirInput.TextColor3 = Colors.textPrimary
projectDirInput.TextSize = 12
projectDirInput.Font = Enum.Font.Gotham
projectDirInput.TextXAlignment = Enum.TextXAlignment.Left
projectDirInput.ClearTextOnFocus = false
projectDirInput.Parent = projectPathFrame

local inputPadding = Instance.new("UIPadding")
inputPadding.PaddingLeft = UDim.new(0, 12)
inputPadding.PaddingRight = UDim.new(0, 12)
inputPadding.Parent = projectDirInput

-- Dropdown toggle button
local dropdownButton = Instance.new("TextButton")
dropdownButton.Size = UDim2.new(0, 36, 1, 0)
dropdownButton.Position = UDim2.new(1, -36, 0, 0)
dropdownButton.BackgroundTransparency = 1
dropdownButton.Text = "▾"
dropdownButton.TextColor3 = Colors.textMuted
dropdownButton.TextSize = 14
dropdownButton.Font = Enum.Font.GothamBold
dropdownButton.AutoButtonColor = true
dropdownButton.Parent = projectPathFrame

-- Dropdown menu frame (hidden by default)
local dropdownFrame = Instance.new("Frame")
dropdownFrame.Size = UDim2.new(1, 0, 0, 0)
dropdownFrame.Position = UDim2.new(0, 0, 1, 4)
dropdownFrame.BackgroundColor3 = Colors.surface
dropdownFrame.BorderSizePixel = 0
dropdownFrame.Visible = false
dropdownFrame.ZIndex = 10
dropdownFrame.Parent = projectPathFrame

local dropdownCorner = Instance.new("UICorner")
dropdownCorner.CornerRadius = UDim.new(0, 6)
dropdownCorner.Parent = dropdownFrame

local dropdownStroke = Instance.new("UIStroke")
dropdownStroke.Color = Colors.textMuted
dropdownStroke.Thickness = 1
dropdownStroke.Transparency = 0.8
dropdownStroke.Parent = dropdownFrame

local dropdownLayout = Instance.new("UIListLayout")
dropdownLayout.SortOrder = Enum.SortOrder.LayoutOrder
dropdownLayout.Padding = UDim.new(0, 2)
dropdownLayout.Parent = dropdownFrame

local dropdownListPadding = Instance.new("UIPadding")
dropdownListPadding.PaddingTop = UDim.new(0, 4)
dropdownListPadding.PaddingBottom = UDim.new(0, 4)
dropdownListPadding.PaddingLeft = UDim.new(0, 4)
dropdownListPadding.PaddingRight = UDim.new(0, 4)
dropdownListPadding.Parent = dropdownFrame

-- Function to update dropdown with recent projects
local function updateRecentProjectsDropdown()
    -- Clear existing items
    for _, child in dropdownFrame:GetChildren() do
        if child:IsA("TextButton") or child:IsA("TextLabel") then
            child:Destroy()
        end
    end

    local settings = Config.load()
    local recentProjects = settings.recentProjects or {}
    local currentPath = Config.getProjectDir()

    if #recentProjects == 0 then
        -- Show "No recent projects" message
        local noProjects = Instance.new("TextLabel")
        noProjects.Size = UDim2.new(1, -8, 0, 24)
        noProjects.BackgroundTransparency = 1
        noProjects.Text = "No recent projects"
        noProjects.TextColor3 = Colors.textMuted
        noProjects.TextSize = 11
        noProjects.Font = Enum.Font.Gotham
        noProjects.TextXAlignment = Enum.TextXAlignment.Left
        noProjects.ZIndex = 11
        noProjects.Parent = dropdownFrame

        dropdownFrame.Size = UDim2.new(1, 0, 0, 32)
        return
    end

    -- Create recent project option buttons
    for i, projectPath in ipairs(recentProjects) do
        local isActive = projectPath == currentPath
        local option = Instance.new("TextButton")
        option.Size = UDim2.new(1, -8, 0, 24)
        option.BackgroundColor3 = isActive and Colors.accent or Colors.surfaceHover
        option.BorderSizePixel = 0
        option.Text = "  " .. projectPath
        option.TextColor3 = isActive and Colors.background or Colors.textSecondary
        option.TextSize = 11
        option.Font = Enum.Font.Gotham
        option.TextXAlignment = Enum.TextXAlignment.Left
        option.TextTruncate = Enum.TextTruncate.AtEnd
        option.AutoButtonColor = true
        option.LayoutOrder = i
        option.ZIndex = 11
        option.Parent = dropdownFrame

        local optionCorner = Instance.new("UICorner")
        optionCorner.CornerRadius = UDim.new(0, 4)
        optionCorner.Parent = option

        option.MouseButton1Click:Connect(function()
            Config.setProjectDir(projectPath)
            projectDirInput.Text = projectPath
            dropdownFrame.Visible = false
            dropdownButton.Text = "▾"
            -- Re-register with server after path change
            if isConnected then
                registerWithServer()
            end
        end)
    end

    -- Set dropdown height based on items
    local itemHeight = 24 + 2 -- height + padding
    local totalHeight = #recentProjects * itemHeight + 8 -- +8 for padding
    dropdownFrame.Size = UDim2.new(1, 0, 0, math.min(totalHeight, 150)) -- Max height 150
end

-- Toggle dropdown
dropdownButton.MouseButton1Click:Connect(function()
    dropdownFrame.Visible = not dropdownFrame.Visible
    dropdownButton.Text = dropdownFrame.Visible and "▴" or "▾"
    if dropdownFrame.Visible then
        updateRecentProjectsDropdown()
    end
end)

projectDirInput.FocusLost:Connect(function()
    if projectDirInput.Text ~= "" then
        Config.setProjectDir(projectDirInput.Text)
        -- Re-register with server after path change
        if isConnected then
            registerWithServer()
        end
    end
end)

-- ============================================================================
-- Action Buttons (side by side)
-- ============================================================================

local actionsSection = Instance.new("Frame")
actionsSection.Size = UDim2.new(1, 0, 0, 56)
actionsSection.BackgroundTransparency = 1
actionsSection.LayoutOrder = 3
actionsSection.Parent = mainFrame

local actionsPadding = Instance.new("UIPadding")
actionsPadding.PaddingTop = UDim.new(0, 0)
actionsPadding.PaddingLeft = UDim.new(0, 16)
actionsPadding.PaddingRight = UDim.new(0, 16)
actionsPadding.Parent = actionsSection

local actionsFrame = Instance.new("Frame")
actionsFrame.Size = UDim2.new(1, 0, 0, 40)
actionsFrame.BackgroundTransparency = 1
actionsFrame.Parent = actionsSection

local actionsLayout = Instance.new("UIListLayout")
actionsLayout.FillDirection = Enum.FillDirection.Horizontal
actionsLayout.SortOrder = Enum.SortOrder.LayoutOrder
actionsLayout.Padding = UDim.new(0, 8)
actionsLayout.Parent = actionsFrame

-- Extract Button (Primary - accent color)
local extractButton = Instance.new("TextButton")
extractButton.Size = UDim2.new(0.5, -4, 1, 0)
extractButton.BackgroundColor3 = Colors.accent
extractButton.BorderSizePixel = 0
extractButton.Text = "📤  Extract"
extractButton.TextColor3 = Colors.background
extractButton.TextSize = 13
extractButton.Font = Enum.Font.GothamBold
extractButton.LayoutOrder = 1
extractButton.AutoButtonColor = true
extractButton.Parent = actionsFrame

local extractCorner = Instance.new("UICorner")
extractCorner.CornerRadius = UDim.new(0, 6)
extractCorner.Parent = extractButton

extractButton.MouseButton1Click:Connect(function()
    if not isConnected then
        warn("Not connected. Click Connect first.")
        return
    end
    if Config.getProjectDir() == "" then
        warn("No project set. Enter path above.")
        return
    end

    -- Check for path mismatch with VS Code
    local hasMismatch, vscodePath = checkPathMismatch()
    if hasMismatch and vscodePath then
        showPathMismatchDialog(
            vscodePath,
            function()  -- onContinue: Extract anyway
                extractGame({ extractTerrain = true })
                changeTracking.studioChanges = 0
                updateButtonBadges()
            end,
            function()  -- onFixPath: Update to VS Code path and extract
                Config.setProjectDir(vscodePath)
                if projectDirInput then
                    projectDirInput.Text = vscodePath
                end
                -- Re-register with server using new path
                registerWithServer()
                -- Now extract
                extractGame({ extractTerrain = true })
                changeTracking.studioChanges = 0
                updateButtonBadges()
            end,
            function()  -- onCancel: Do nothing
            end
        )
        return
    end

    -- No mismatch, proceed directly
    extractGame({ extractTerrain = true })
    -- Reset studio changes after extract
    changeTracking.studioChanges = 0
    updateButtonBadges()
end)

-- Sync Button (Secondary - surface with stroke)
local syncButton = Instance.new("TextButton")
syncButton.Size = UDim2.new(0.5, -4, 1, 0)
syncButton.BackgroundColor3 = Colors.surface
syncButton.BorderSizePixel = 0
syncButton.Text = "📥  Sync"
syncButton.TextColor3 = Colors.textPrimary
syncButton.TextSize = 13
syncButton.Font = Enum.Font.GothamBold
syncButton.LayoutOrder = 2
syncButton.AutoButtonColor = true
syncButton.Parent = actionsFrame

local syncCorner = Instance.new("UICorner")
syncCorner.CornerRadius = UDim.new(0, 6)
syncCorner.Parent = syncButton

local syncStroke = Instance.new("UIStroke")
syncStroke.Color = Colors.textMuted
syncStroke.Thickness = 1
syncStroke.Transparency = 0.8
syncStroke.Parent = syncButton

-- Define button state update functions
updateSyncButtonState = function(state: string)
    currentSyncState = state

    -- Clean up any existing pulse animation
    if syncPulseConnection then
        syncPulseConnection:Disconnect()
        syncPulseConnection = nil
    end

    -- Reset transparency
    syncButton.BackgroundTransparency = 0

    if state == SyncButtonState.IDLE then
        -- Stop timer if running
        operationState.syncStartTime = nil
        -- Ready state: subtle secondary style
        syncButton.BackgroundColor3 = Colors.surface
        syncButton.TextColor3 = Colors.textPrimary
        -- Show last sync time if available
        if operationState.lastSyncDuration and operationState.lastSyncCount then
            syncButton.Text = string.format("📥  Sync (%.1fs)", operationState.lastSyncDuration)
        else
            syncButton.Text = "📥  Sync"
        end
        syncStroke.Color = Colors.textMuted
        syncStroke.Transparency = 0.8

    elseif state == SyncButtonState.SYNCING then
        -- Active syncing: accent color with timer
        syncButton.BackgroundColor3 = Colors.accent
        syncButton.TextColor3 = Colors.background
        syncStroke.Color = Colors.accent
        syncStroke.Transparency = 0

        -- Start timer update
        operationState.syncStartTime = os.clock()
        if operationState.timerThread then
            task.cancel(operationState.timerThread)
        end
        operationState.timerThread = task.spawn(function()
            while operationState.syncStartTime do
                local elapsed = os.clock() - operationState.syncStartTime
                syncButton.Text = string.format("📥  %.1fs", elapsed)
                task.wait(0.1)
            end
        end)

    elseif state == SyncButtonState.DISCONNECTED then
        -- Stop timer if running
        operationState.syncStartTime = nil
        -- Disabled/disconnected: muted appearance
        syncButton.BackgroundColor3 = Colors.surface
        syncButton.TextColor3 = Colors.textMuted
        syncButton.Text = "📥  Sync"
        syncStroke.Color = Colors.textMuted
        syncStroke.Transparency = 0.9
        syncButton.BackgroundTransparency = 0.5

    elseif state == SyncButtonState.SUCCESS then
        -- Save duration before stopping timer
        if operationState.syncStartTime then
            operationState.lastSyncDuration = os.clock() - operationState.syncStartTime
        end
        operationState.syncStartTime = nil
        -- Brief success flash with final time
        syncButton.BackgroundColor3 = Colors.success
        syncButton.TextColor3 = Colors.background
        if operationState.lastSyncDuration then
            syncButton.Text = string.format("✓  %.1fs", operationState.lastSyncDuration)
        else
            syncButton.Text = "✓  Done"
        end
        syncStroke.Color = Colors.success
        syncStroke.Transparency = 0

        -- Return to idle after 2s
        task.delay(2, function()
            if currentSyncState == SyncButtonState.SUCCESS then
                updateSyncButtonState(SyncButtonState.IDLE)
            end
        end)
    end
end

updateExtractButtonState = function(connected: boolean)
    if connected then
        extractButton.BackgroundColor3 = Colors.accent
        extractButton.TextColor3 = Colors.background
        extractButton.BackgroundTransparency = 0
    else
        extractButton.BackgroundColor3 = Colors.accent
        extractButton.TextColor3 = Colors.background
        extractButton.BackgroundTransparency = 0.5
    end
end

-- Update button text to show change counts
updateButtonBadges = function()
    -- Update Extract button
    if changeTracking.studioChanges > 0 then
        extractButton.Text = "📤 Extract (" .. changeTracking.studioChanges .. ")"
    else
        extractButton.Text = "📤 Extract"
    end

    -- Update Sync button (only when idle)
    if currentSyncState == SyncButtonState.IDLE or currentSyncState == SyncButtonState.DISCONNECTED then
        if changeTracking.localChanges > 0 then
            syncButton.Text = "📥 Sync (" .. changeTracking.localChanges .. ")"
        else
            syncButton.Text = "📥 Sync"
        end
    end
end

-- Fetch pending local changes from server
local function fetchLocalChanges()
    if not isConnected then
        changeTracking.localChanges = 0
        return
    end

    local projectDir = Config.getProjectDir()
    if projectDir == "" then
        changeTracking.localChanges = 0
        return
    end

    local success, result = httpPost("/sync/pending-changes", {
        project_dir = projectDir,
    })

    if success and result and result.count then
        changeTracking.localChanges = result.count
    else
        changeTracking.localChanges = 0
    end

    updateButtonBadges()
end

syncButton.MouseButton1Click:Connect(function()
    if not isConnected then
        warn("Not connected. Click Connect first.")
        return
    end
    local projectDir = Config.getProjectDir()
    if projectDir == "" then
        warn("No project set. Enter path above.")
        return
    end
    if isSyncing then
        return
    end

    -- Show syncing state
    updateSyncButtonState(SyncButtonState.SYNCING)

    -- Request file tree from server
    local success, result = httpPost("/sync/read-tree", {
        project_dir = projectDir,
    })

    if not success or not result.success then
        warn("Can't read files. " .. (result and result.error or "Try again."))
        updateSyncButtonState(SyncButtonState.IDLE)
        return
    end

    local instances = result.instances
    if not instances or #instances == 0 then
        updateSyncButtonState(SyncButtonState.IDLE)
        return
    end

    -- Convert to operations
    local operations = {}
    for _, inst in ipairs(instances) do
        if inst.path and inst.path ~= "" then
            table.insert(operations, { type = "update", path = inst.path, data = inst })
        end
    end

    -- Sort by depth (shallower paths first so parents are created before children)
    table.sort(operations, function(a, b)
        return select(2, string.gsub(a.path, "/", "")) < select(2, string.gsub(b.path, "/", ""))
    end)

    syncBatch({ operations = operations })

    -- Delete orphans (instances in Studio not in files)
    local filePaths: {[string]: boolean} = {}
    for _, inst in ipairs(instances) do
        if inst.path and inst.path ~= "" then
            filePaths[inst.path] = true
        end
    end

    -- Helper to escape name for path (same logic as Serializer)
    local function escapeNameForPath(name: string): string
        return string.gsub(name, "/", "[SLASH]")
    end

    -- Helper to build path for an instance
    local function buildInstancePath(instance: Instance): string?
        local pathParts: {string} = {}
        local current = instance
        -- Walk up to game, collecting names
        while current and current ~= game do
            pathParts[#pathParts + 1] = escapeNameForPath(current.Name)
            if current.Parent then
                current = current.Parent :: Instance
            else
                break
            end
        end
        if #pathParts == 0 then
            return nil
        end
        -- Reverse the array (we built it bottom-up)
        local reversed: {string} = {}
        for i = #pathParts, 1, -1 do
            reversed[#reversed + 1] = pathParts[i]
        end
        return table.concat(reversed, "/")
    end

    -- Services to check for orphans
    local trackedServices = {
        "Workspace", "ReplicatedStorage", "ReplicatedFirst",
        "ServerScriptService", "ServerStorage", "StarterGui",
        "StarterPack", "StarterPlayer", "Lighting", "SoundService",
        "Teams", "Chat", "LocalizationService", "TestService",
        "MaterialService",
    }

    -- Collect all orphans with their depths
    local orphans: {{instance: Instance, path: string, depth: number}} = {}
    for _, serviceName in trackedServices do
        local service = game:FindFirstChild(serviceName)
        if service then
            for _, descendant in service:GetDescendants() do
                -- Skip Terrain (handled separately)
                if descendant.ClassName == "Terrain" then
                    continue
                end
                local path = buildInstancePath(descendant)
                if path and not filePaths[path] then
                    local depth = select(2, string.gsub(path, "/", ""))
                    table.insert(orphans, { instance = descendant, path = path, depth = depth })
                end
            end
        end
    end

    -- Sort by depth descending (delete children before parents)
    table.sort(orphans, function(a, b)
        return a.depth > b.depth
    end)

    -- Delete orphans
    local deletedCount = 0
    for _, orphan in orphans do
        -- Check if instance still exists (parent might have been deleted)
        if orphan.instance and orphan.instance.Parent then
            pcall(function()
                orphan.instance:Destroy()
            end)
            deletedCount += 1
        end
    end

    if deletedCount > 0 then
        print(string.format("[RbxSync] Deleted %d orphan instances", deletedCount))
    end

    -- Sync terrain if available
    local terrainSuccess, terrainResult = httpPost("/sync/read-terrain", {
        project_dir = projectDir,
    })

    print(string.format("[RbxSync] Terrain read result: success=%s, hasTerrain=%s, hasData=%s",
        tostring(terrainSuccess),
        tostring(terrainResult and terrainResult.success and terrainResult.hasTerrain),
        tostring(terrainResult and terrainResult.terrain ~= nil)))

    if terrainSuccess and terrainResult.success and terrainResult.hasTerrain and terrainResult.terrain then
        print("[RbxSync] Applying terrain...")
        local applied, chunkCount = TerrainHandler.applyTerrain(terrainResult.terrain)
        if applied then
            print(string.format("[RbxSync] Terrain applied: %d chunks", chunkCount))
        else
            warn("[RbxSync] Failed to apply terrain")
        end
    elseif terrainResult and terrainResult.error then
        warn("[RbxSync] Terrain read error:", terrainResult.error)
    end

    -- Reset local changes after sync
    changeTracking.localChanges = 0
    updateButtonBadges()

    -- Show success state
    updateSyncButtonState(SyncButtonState.SUCCESS)
end)

-- ============================================================================
-- Project Info Section
-- ============================================================================

local infoSection = Instance.new("Frame")
infoSection.Size = UDim2.new(1, 0, 0, 88)
infoSection.BackgroundTransparency = 1
infoSection.LayoutOrder = 4
infoSection.Parent = mainFrame

local infoSectionPadding = Instance.new("UIPadding")
infoSectionPadding.PaddingTop = UDim.new(0, 0)
infoSectionPadding.PaddingLeft = UDim.new(0, 16)
infoSectionPadding.PaddingRight = UDim.new(0, 16)
infoSectionPadding.PaddingBottom = UDim.new(0, 16)
infoSectionPadding.Parent = infoSection

local infoFrame = Instance.new("Frame")
infoFrame.Size = UDim2.new(1, 0, 1, 0)
infoFrame.BackgroundColor3 = Colors.surface
infoFrame.BorderSizePixel = 0
infoFrame.Parent = infoSection

local infoFrameCorner = Instance.new("UICorner")
infoFrameCorner.CornerRadius = UDim.new(0, 6)
infoFrameCorner.Parent = infoFrame

local infoLayout = Instance.new("UIListLayout")
infoLayout.SortOrder = Enum.SortOrder.LayoutOrder
infoLayout.Padding = UDim.new(0, 4)
infoLayout.Parent = infoFrame

local infoPadding = Instance.new("UIPadding")
infoPadding.PaddingTop = UDim.new(0, 12)
infoPadding.PaddingLeft = UDim.new(0, 12)
infoPadding.PaddingRight = UDim.new(0, 12)
infoPadding.Parent = infoFrame

-- Place name line
local placeNameLine = Instance.new("TextLabel")
placeNameLine.Size = UDim2.new(1, 0, 0, 18)
placeNameLine.BackgroundTransparency = 1
placeNameLine.Text = "Loading..."
placeNameLine.TextColor3 = Colors.textPrimary
placeNameLine.TextSize = 13
placeNameLine.Font = Enum.Font.GothamMedium
placeNameLine.TextXAlignment = Enum.TextXAlignment.Left
placeNameLine.TextTruncate = Enum.TextTruncate.AtEnd
placeNameLine.LayoutOrder = 1
placeNameLine.Parent = infoFrame

-- Instance count line
local instanceLine = Instance.new("TextLabel")
instanceLine.Size = UDim2.new(1, 0, 0, 14)
instanceLine.BackgroundTransparency = 1
instanceLine.Text = "..."
instanceLine.TextColor3 = Colors.textMuted
instanceLine.TextSize = 11
instanceLine.Font = Enum.Font.Gotham
instanceLine.TextXAlignment = Enum.TextXAlignment.Left
instanceLine.LayoutOrder = 2
instanceLine.Parent = infoFrame

-- Optimization stats line (lights, meshes, sounds, etc.)
local optimizationLine = Instance.new("TextLabel")
optimizationLine.Size = UDim2.new(1, 0, 0, 14)
optimizationLine.BackgroundTransparency = 1
optimizationLine.Text = ""
optimizationLine.TextColor3 = Colors.textMuted
optimizationLine.TextSize = 10
optimizationLine.Font = Enum.Font.Gotham
optimizationLine.TextXAlignment = Enum.TextXAlignment.Left
optimizationLine.LayoutOrder = 3
optimizationLine.Parent = infoFrame

-- Status line (dynamic updates)
local statusLine = Instance.new("TextLabel")
statusLine.Size = UDim2.new(1, 0, 0, 14)
statusLine.BackgroundTransparency = 1
statusLine.Text = ""
statusLine.TextColor3 = Colors.accent
statusLine.TextSize = 11
statusLine.Font = Enum.Font.GothamMedium
statusLine.TextXAlignment = Enum.TextXAlignment.Left
statusLine.LayoutOrder = 3
statusLine.Parent = infoFrame

-- Status update function (updates forward declaration)
local statusClearTimeout: thread? = nil
setStatus = function(text: string, color: Color3?, duration: number?)
    statusLine.Text = text
    statusLine.TextColor3 = color or Colors.accent

    -- Clear previous timeout
    if statusClearTimeout then
        task.cancel(statusClearTimeout)
    end

    -- Auto-clear after duration (default 5s)
    local clearAfter = duration or 5
    if clearAfter > 0 then
        statusClearTimeout = task.delay(clearAfter, function()
            statusLine.Text = ""
        end)
    end
end

-- Function to update project info display
local function updateInfoDisplay()
    -- Update place name
    local placeName = getPlaceName()
    local placeId = game.PlaceId
    if placeId > 0 then
        placeNameLine.Text = placeName .. " (" .. tostring(placeId) .. ")"
    else
        placeNameLine.Text = placeName .. " (unsaved)"
    end

    -- Update instance breakdown (classes instead of just count)
    instanceLine.Text = getClassBreakdownString()

    -- Update optimization stats
    local optStats = getOptimizationStatsString()
    optimizationLine.Text = optStats
    optimizationLine.Visible = optStats ~= ""
end

-- ============================================================================
-- Sync Options Section
-- ============================================================================

local syncOptionsSection = Instance.new("Frame")
syncOptionsSection.Size = UDim2.new(1, 0, 0, 80)
syncOptionsSection.BackgroundTransparency = 1
syncOptionsSection.LayoutOrder = 5
syncOptionsSection.Parent = mainFrame

local syncOptionsPadding = Instance.new("UIPadding")
syncOptionsPadding.PaddingTop = UDim.new(0, 0)
syncOptionsPadding.PaddingLeft = UDim.new(0, 16)
syncOptionsPadding.PaddingRight = UDim.new(0, 16)
syncOptionsPadding.Parent = syncOptionsSection

local syncOptionsLabel = Instance.new("TextLabel")
syncOptionsLabel.Size = UDim2.new(1, 0, 0, 16)
syncOptionsLabel.BackgroundTransparency = 1
syncOptionsLabel.Text = "LIVE SYNC"
syncOptionsLabel.TextColor3 = Colors.textMuted
syncOptionsLabel.TextSize = 10
syncOptionsLabel.Font = Enum.Font.GothamBold
syncOptionsLabel.TextXAlignment = Enum.TextXAlignment.Left
syncOptionsLabel.Parent = syncOptionsSection

-- Path mismatch check and dialog
local pathMismatchOverlay: Frame? = nil

checkPathMismatch = function(): (boolean, string?)
    -- Get VS Code workspaces from server
    local success, data = httpGet("/rbxsync/workspaces")
    if not success or not data or not data.workspaces then
        return false, nil  -- No mismatch if we can't check
    end

    local studioPath = Config.getProjectDir()
    if studioPath == "" then
        return false, nil  -- No path set yet
    end

    local vscodeWorkspaces = data.workspaces
    if #vscodeWorkspaces == 0 then
        return false, nil  -- No VS Code connected
    end

    -- Check if Studio path matches any VS Code workspace (or is parent/child)
    for _, vscodePath in ipairs(vscodeWorkspaces) do
        if studioPath == vscodePath then
            return false, nil  -- Exact match
        end
        -- Check if one is parent of other
        if string.find(studioPath, vscodePath, 1, true) == 1 or
           string.find(vscodePath, studioPath, 1, true) == 1 then
            return false, nil  -- Parent/child relationship is OK
        end
    end

    -- Mismatch found
    return true, vscodeWorkspaces[1]
end

showPathMismatchDialog = function(vscodePath: string, onContinue: () -> (), onFixPath: () -> (), onCancel: () -> ())
    -- Remove existing overlay if present
    if pathMismatchOverlay then
        pathMismatchOverlay:Destroy()
    end

    local studioPath = Config.getProjectDir()

    -- Create overlay
    local overlay = Instance.new("Frame")
    overlay.Name = "PathMismatchOverlay"
    overlay.Size = UDim2.new(1, 0, 1, 0)
    overlay.Position = UDim2.new(0, 0, 0, 0)
    overlay.BackgroundColor3 = Color3.fromRGB(0, 0, 0)
    overlay.BackgroundTransparency = 0.4
    overlay.ZIndex = 100
    overlay.Parent = mainFrame
    pathMismatchOverlay = overlay

    -- Dialog box (taller for more content)
    local dialog = Instance.new("Frame")
    dialog.Name = "Dialog"
    dialog.Size = UDim2.new(1, -16, 0, 180)
    dialog.Position = UDim2.new(0.5, 0, 0.5, 0)
    dialog.AnchorPoint = Vector2.new(0.5, 0.5)
    dialog.BackgroundColor3 = Colors.surface
    dialog.BorderSizePixel = 0
    dialog.ZIndex = 101
    dialog.Parent = overlay

    local dialogCorner = Instance.new("UICorner")
    dialogCorner.CornerRadius = UDim.new(0, 8)
    dialogCorner.Parent = dialog

    local dialogStroke = Instance.new("UIStroke")
    dialogStroke.Color = Color3.fromRGB(255, 180, 0)  -- Warning color
    dialogStroke.Thickness = 2
    dialogStroke.Parent = dialog

    -- Warning icon and title
    local titleLabel = Instance.new("TextLabel")
    titleLabel.Size = UDim2.new(1, -16, 0, 20)
    titleLabel.Position = UDim2.new(0, 8, 0, 8)
    titleLabel.BackgroundTransparency = 1
    titleLabel.Text = "⚠️ Path Mismatch"
    titleLabel.TextColor3 = Color3.fromRGB(255, 180, 0)
    titleLabel.TextSize = 14
    titleLabel.Font = Enum.Font.GothamBold
    titleLabel.TextXAlignment = Enum.TextXAlignment.Left
    titleLabel.ZIndex = 102
    titleLabel.Parent = dialog

    -- Message
    local messageLabel = Instance.new("TextLabel")
    messageLabel.Size = UDim2.new(1, -16, 0, 70)
    messageLabel.Position = UDim2.new(0, 8, 0, 32)
    messageLabel.BackgroundTransparency = 1
    messageLabel.Text = string.format(
        "Studio path: %s\n\nVS Code path: %s\n\nFiles will extract to the Studio path, not VS Code!",
        studioPath,
        vscodePath
    )
    messageLabel.TextColor3 = Colors.textSecondary
    messageLabel.TextSize = 10
    messageLabel.Font = Enum.Font.Gotham
    messageLabel.TextXAlignment = Enum.TextXAlignment.Left
    messageLabel.TextYAlignment = Enum.TextYAlignment.Top
    messageLabel.TextWrapped = true
    messageLabel.ZIndex = 102
    messageLabel.Parent = dialog

    -- Button container
    local buttonContainer = Instance.new("Frame")
    buttonContainer.Size = UDim2.new(1, -16, 0, 50)
    buttonContainer.Position = UDim2.new(0, 8, 1, -58)
    buttonContainer.BackgroundTransparency = 1
    buttonContainer.ZIndex = 102
    buttonContainer.Parent = dialog

    -- Use VS Code Path button (primary action)
    local fixBtn = Instance.new("TextButton")
    fixBtn.Size = UDim2.new(1, 0, 0, 22)
    fixBtn.Position = UDim2.new(0, 0, 0, 0)
    fixBtn.BackgroundColor3 = Colors.accent
    fixBtn.BorderSizePixel = 0
    fixBtn.Text = "Use VS Code Path"
    fixBtn.TextColor3 = Colors.background
    fixBtn.TextSize = 11
    fixBtn.Font = Enum.Font.GothamBold
    fixBtn.ZIndex = 103
    fixBtn.Parent = buttonContainer

    local fixCorner = Instance.new("UICorner")
    fixCorner.CornerRadius = UDim.new(0, 4)
    fixCorner.Parent = fixBtn

    -- Continue Anyway and Cancel buttons (side by side)
    local continueBtn = Instance.new("TextButton")
    continueBtn.Size = UDim2.new(0.48, 0, 0, 22)
    continueBtn.Position = UDim2.new(0, 0, 0, 26)
    continueBtn.BackgroundColor3 = Colors.surface
    continueBtn.BorderSizePixel = 0
    continueBtn.Text = "Extract Anyway"
    continueBtn.TextColor3 = Colors.textSecondary
    continueBtn.TextSize = 10
    continueBtn.Font = Enum.Font.GothamMedium
    continueBtn.ZIndex = 103
    continueBtn.Parent = buttonContainer

    local continueCorner = Instance.new("UICorner")
    continueCorner.CornerRadius = UDim.new(0, 4)
    continueCorner.Parent = continueBtn

    local continueStroke = Instance.new("UIStroke")
    continueStroke.Color = Colors.textMuted
    continueStroke.Thickness = 1
    continueStroke.Parent = continueBtn

    local cancelBtn = Instance.new("TextButton")
    cancelBtn.Size = UDim2.new(0.48, 0, 0, 22)
    cancelBtn.Position = UDim2.new(0.52, 0, 0, 26)
    cancelBtn.BackgroundColor3 = Colors.surface
    cancelBtn.BorderSizePixel = 0
    cancelBtn.Text = "Cancel"
    cancelBtn.TextColor3 = Colors.textSecondary
    cancelBtn.TextSize = 10
    cancelBtn.Font = Enum.Font.GothamMedium
    cancelBtn.ZIndex = 103
    cancelBtn.Parent = buttonContainer

    local cancelCorner = Instance.new("UICorner")
    cancelCorner.CornerRadius = UDim.new(0, 4)
    cancelCorner.Parent = cancelBtn

    local cancelStroke = Instance.new("UIStroke")
    cancelStroke.Color = Colors.textMuted
    cancelStroke.Thickness = 1
    cancelStroke.Parent = cancelBtn

    -- Button handlers
    fixBtn.MouseButton1Click:Connect(function()
        overlay:Destroy()
        pathMismatchOverlay = nil
        onFixPath()
    end)

    continueBtn.MouseButton1Click:Connect(function()
        overlay:Destroy()
        pathMismatchOverlay = nil
        onContinue()
    end)

    cancelBtn.MouseButton1Click:Connect(function()
        overlay:Destroy()
        pathMismatchOverlay = nil
        onCancel()
    end)
end

-- Confirmation dialog helper
local confirmationOverlay: Frame? = nil

local function showConfirmation(title: string, message: string, onConfirm: () -> (), onCancel: () -> ())
    -- Remove existing overlay if present
    if confirmationOverlay then
        confirmationOverlay:Destroy()
    end

    -- Create overlay
    local overlay = Instance.new("Frame")
    overlay.Name = "ConfirmationOverlay"
    overlay.Size = UDim2.new(1, 0, 1, 0)
    overlay.Position = UDim2.new(0, 0, 0, 0)
    overlay.BackgroundColor3 = Color3.fromRGB(0, 0, 0)
    overlay.BackgroundTransparency = 0.4
    overlay.ZIndex = 100
    overlay.Parent = mainFrame
    confirmationOverlay = overlay

    -- Dialog box
    local dialog = Instance.new("Frame")
    dialog.Name = "Dialog"
    dialog.Size = UDim2.new(1, -32, 0, 120)
    dialog.Position = UDim2.new(0.5, 0, 0.5, 0)
    dialog.AnchorPoint = Vector2.new(0.5, 0.5)
    dialog.BackgroundColor3 = Colors.surface
    dialog.BorderSizePixel = 0
    dialog.ZIndex = 101
    dialog.Parent = overlay

    local dialogCorner = Instance.new("UICorner")
    dialogCorner.CornerRadius = UDim.new(0, 8)
    dialogCorner.Parent = dialog

    local dialogStroke = Instance.new("UIStroke")
    dialogStroke.Color = Colors.border
    dialogStroke.Thickness = 1
    dialogStroke.Parent = dialog

    -- Title
    local titleLabel = Instance.new("TextLabel")
    titleLabel.Size = UDim2.new(1, -16, 0, 20)
    titleLabel.Position = UDim2.new(0, 8, 0, 8)
    titleLabel.BackgroundTransparency = 1
    titleLabel.Text = title
    titleLabel.TextColor3 = Colors.textPrimary
    titleLabel.TextSize = 13
    titleLabel.Font = Enum.Font.GothamBold
    titleLabel.TextXAlignment = Enum.TextXAlignment.Left
    titleLabel.ZIndex = 102
    titleLabel.Parent = dialog

    -- Message
    local messageLabel = Instance.new("TextLabel")
    messageLabel.Size = UDim2.new(1, -16, 0, 36)
    messageLabel.Position = UDim2.new(0, 8, 0, 30)
    messageLabel.BackgroundTransparency = 1
    messageLabel.Text = message
    messageLabel.TextColor3 = Colors.textSecondary
    messageLabel.TextSize = 11
    messageLabel.Font = Enum.Font.Gotham
    messageLabel.TextXAlignment = Enum.TextXAlignment.Left
    messageLabel.TextYAlignment = Enum.TextYAlignment.Top
    messageLabel.TextWrapped = true
    messageLabel.ZIndex = 102
    messageLabel.Parent = dialog

    -- Button container
    local buttonContainer = Instance.new("Frame")
    buttonContainer.Size = UDim2.new(1, -16, 0, 28)
    buttonContainer.Position = UDim2.new(0, 8, 1, -36)
    buttonContainer.BackgroundTransparency = 1
    buttonContainer.ZIndex = 102
    buttonContainer.Parent = dialog

    -- Cancel button
    local cancelBtn = Instance.new("TextButton")
    cancelBtn.Size = UDim2.new(0.48, 0, 1, 0)
    cancelBtn.Position = UDim2.new(0, 0, 0, 0)
    cancelBtn.BackgroundColor3 = Colors.surface
    cancelBtn.BorderSizePixel = 0
    cancelBtn.Text = "Cancel"
    cancelBtn.TextColor3 = Colors.textSecondary
    cancelBtn.TextSize = 11
    cancelBtn.Font = Enum.Font.GothamMedium
    cancelBtn.ZIndex = 103
    cancelBtn.Parent = buttonContainer

    local cancelCorner = Instance.new("UICorner")
    cancelCorner.CornerRadius = UDim.new(0, 4)
    cancelCorner.Parent = cancelBtn

    local cancelStroke = Instance.new("UIStroke")
    cancelStroke.Color = Colors.textMuted
    cancelStroke.Thickness = 1
    cancelStroke.Parent = cancelBtn

    -- Confirm button
    local confirmBtn = Instance.new("TextButton")
    confirmBtn.Size = UDim2.new(0.48, 0, 1, 0)
    confirmBtn.Position = UDim2.new(0.52, 0, 0, 0)
    confirmBtn.BackgroundColor3 = Colors.accent
    confirmBtn.BorderSizePixel = 0
    confirmBtn.Text = "Sync Now"
    confirmBtn.TextColor3 = Colors.background
    confirmBtn.TextSize = 11
    confirmBtn.Font = Enum.Font.GothamMedium
    confirmBtn.ZIndex = 103
    confirmBtn.Parent = buttonContainer

    local confirmCorner = Instance.new("UICorner")
    confirmCorner.CornerRadius = UDim.new(0, 4)
    confirmCorner.Parent = confirmBtn

    -- Button handlers
    cancelBtn.MouseButton1Click:Connect(function()
        overlay:Destroy()
        confirmationOverlay = nil
        onCancel()
    end)

    confirmBtn.MouseButton1Click:Connect(function()
        overlay:Destroy()
        confirmationOverlay = nil
        onConfirm()
    end)

    -- Click outside to cancel
    overlay.InputBegan:Connect(function(input)
        if input.UserInputType == Enum.UserInputType.MouseButton1 then
            local pos = input.Position
            local dialogPos = dialog.AbsolutePosition
            local dialogSize = dialog.AbsoluteSize
            if pos.X < dialogPos.X or pos.X > dialogPos.X + dialogSize.X or
               pos.Y < dialogPos.Y or pos.Y > dialogPos.Y + dialogSize.Y then
                overlay:Destroy()
                confirmationOverlay = nil
                onCancel()
            end
        end
    end)
end

-- Connect prompt overlay
local connectPromptOverlay: Frame? = nil

local function showConnectPrompt()
    -- Don't show if already shown or already connected
    if hasShownConnectPrompt or isConnected then
        return
    end
    hasShownConnectPrompt = true

    -- Remove existing overlay if present
    if connectPromptOverlay then
        connectPromptOverlay:Destroy()
    end

    -- Create overlay
    local overlay = Instance.new("Frame")
    overlay.Name = "ConnectPromptOverlay"
    overlay.Size = UDim2.new(1, 0, 1, 0)
    overlay.Position = UDim2.new(0, 0, 0, 0)
    overlay.BackgroundColor3 = Color3.fromRGB(0, 0, 0)
    overlay.BackgroundTransparency = 0.4
    overlay.ZIndex = 100
    overlay.Parent = mainFrame
    connectPromptOverlay = overlay

    -- Dialog box
    local dialog = Instance.new("Frame")
    dialog.Name = "Dialog"
    dialog.Size = UDim2.new(1, -32, 0, 140)
    dialog.Position = UDim2.new(0.5, 0, 0.5, 0)
    dialog.AnchorPoint = Vector2.new(0.5, 0.5)
    dialog.BackgroundColor3 = Colors.surface
    dialog.BorderSizePixel = 0
    dialog.ZIndex = 101
    dialog.Parent = overlay

    local dialogCorner = Instance.new("UICorner")
    dialogCorner.CornerRadius = UDim.new(0, 8)
    dialogCorner.Parent = dialog

    local dialogStroke = Instance.new("UIStroke")
    dialogStroke.Color = Colors.border
    dialogStroke.Thickness = 1
    dialogStroke.Parent = dialog

    -- Title
    local titleLabel = Instance.new("TextLabel")
    titleLabel.Size = UDim2.new(1, -16, 0, 24)
    titleLabel.Position = UDim2.new(0, 8, 0, 12)
    titleLabel.BackgroundTransparency = 1
    titleLabel.Text = "Connect to RbxSync?"
    titleLabel.TextColor3 = Colors.textPrimary
    titleLabel.TextSize = 14
    titleLabel.Font = Enum.Font.GothamBold
    titleLabel.TextXAlignment = Enum.TextXAlignment.Left
    titleLabel.ZIndex = 102
    titleLabel.Parent = dialog

    -- Message
    local messageLabel = Instance.new("TextLabel")
    messageLabel.Size = UDim2.new(1, -16, 0, 40)
    messageLabel.Position = UDim2.new(0, 8, 0, 38)
    messageLabel.BackgroundTransparency = 1
    messageLabel.Text = "RbxSync server detected. Connect to sync files between Studio and your local filesystem."
    messageLabel.TextColor3 = Colors.textSecondary
    messageLabel.TextSize = 11
    messageLabel.Font = Enum.Font.Gotham
    messageLabel.TextXAlignment = Enum.TextXAlignment.Left
    messageLabel.TextYAlignment = Enum.TextYAlignment.Top
    messageLabel.TextWrapped = true
    messageLabel.ZIndex = 102
    messageLabel.Parent = dialog

    -- Button container
    local buttonContainer = Instance.new("Frame")
    buttonContainer.Size = UDim2.new(1, -16, 0, 28)
    buttonContainer.Position = UDim2.new(0, 8, 1, -40)
    buttonContainer.BackgroundTransparency = 1
    buttonContainer.ZIndex = 102
    buttonContainer.Parent = dialog

    -- Dismiss button
    local dismissBtn = Instance.new("TextButton")
    dismissBtn.Size = UDim2.new(0.48, 0, 1, 0)
    dismissBtn.Position = UDim2.new(0, 0, 0, 0)
    dismissBtn.BackgroundColor3 = Colors.surface
    dismissBtn.BorderSizePixel = 0
    dismissBtn.Text = "Dismiss"
    dismissBtn.TextColor3 = Colors.textSecondary
    dismissBtn.TextSize = 11
    dismissBtn.Font = Enum.Font.GothamMedium
    dismissBtn.ZIndex = 103
    dismissBtn.Parent = buttonContainer

    local dismissCorner = Instance.new("UICorner")
    dismissCorner.CornerRadius = UDim.new(0, 4)
    dismissCorner.Parent = dismissBtn

    local dismissStroke = Instance.new("UIStroke")
    dismissStroke.Color = Colors.textMuted
    dismissStroke.Thickness = 1
    dismissStroke.Parent = dismissBtn

    -- Connect button (primary)
    local connectBtn = Instance.new("TextButton")
    connectBtn.Size = UDim2.new(0.48, 0, 1, 0)
    connectBtn.Position = UDim2.new(0.52, 0, 0, 0)
    connectBtn.BackgroundColor3 = Colors.accent
    connectBtn.BorderSizePixel = 0
    connectBtn.Text = "Connect"
    connectBtn.TextColor3 = Colors.background
    connectBtn.TextSize = 11
    connectBtn.Font = Enum.Font.GothamMedium
    connectBtn.ZIndex = 103
    connectBtn.Parent = buttonContainer

    local connectCorner = Instance.new("UICorner")
    connectCorner.CornerRadius = UDim.new(0, 4)
    connectCorner.Parent = connectBtn

    -- Button handlers
    dismissBtn.MouseButton1Click:Connect(function()
        overlay:Destroy()
        connectPromptOverlay = nil
        -- User dismissed - they can manually connect later via the connect button
    end)

    connectBtn.MouseButton1Click:Connect(function()
        overlay:Destroy()
        connectPromptOverlay = nil
        -- Enable connection
        userDisconnected = false
        -- Trigger immediate connection attempt
        local success, _ = httpGet("/health")
        if success then
            isConnected = true
            print("Connected")
            registerWithServer()
            if syncSettings.studioToFiles then
                ChangeTracker.start()
            end
            startConsoleCapture()
            -- Update UI to show connected state
            updateConnectionUI()
        else
            warn("Could not connect to server")
        end
    end)
end

-- Helper function to create a checkbox with confirmation support
local function createCheckbox(parent: Frame, yOffset: number, text: string, checked: boolean, onBeforeChange: ((boolean) -> boolean)?, onChange: (boolean) -> ())
    local checkboxFrame = Instance.new("Frame")
    checkboxFrame.Size = UDim2.new(1, 0, 0, 22)
    checkboxFrame.Position = UDim2.new(0, 0, 0, yOffset)
    checkboxFrame.BackgroundTransparency = 1
    checkboxFrame.Parent = parent

    local checkboxButton = Instance.new("TextButton")
    checkboxButton.Size = UDim2.new(0, 16, 0, 16)
    checkboxButton.Position = UDim2.new(0, 0, 0.5, -8)
    checkboxButton.BackgroundColor3 = checked and Colors.accent or Colors.surface
    checkboxButton.BorderSizePixel = 0
    checkboxButton.Text = checked and "✓" or ""
    checkboxButton.TextColor3 = Colors.background
    checkboxButton.TextSize = 12
    checkboxButton.Font = Enum.Font.GothamBold
    checkboxButton.Parent = checkboxFrame

    local checkboxCorner = Instance.new("UICorner")
    checkboxCorner.CornerRadius = UDim.new(0, 3)
    checkboxCorner.Parent = checkboxButton

    local checkboxStroke = Instance.new("UIStroke")
    checkboxStroke.Color = checked and Colors.accent or Colors.textMuted
    checkboxStroke.Thickness = 1
    checkboxStroke.Parent = checkboxButton

    local checkboxLabel = Instance.new("TextLabel")
    checkboxLabel.Size = UDim2.new(1, -24, 1, 0)
    checkboxLabel.Position = UDim2.new(0, 24, 0, 0)
    checkboxLabel.BackgroundTransparency = 1
    checkboxLabel.Text = text
    checkboxLabel.TextColor3 = Colors.textSecondary
    checkboxLabel.TextSize = 11
    checkboxLabel.Font = Enum.Font.Gotham
    checkboxLabel.TextXAlignment = Enum.TextXAlignment.Left
    checkboxLabel.Parent = checkboxFrame

    local isChecked = checked

    -- Function to update visual state
    local function updateVisual(newState: boolean)
        isChecked = newState
        checkboxButton.BackgroundColor3 = isChecked and Colors.accent or Colors.surface
        checkboxButton.Text = isChecked and "✓" or ""
        checkboxStroke.Color = isChecked and Colors.accent or Colors.textMuted
    end

    checkboxButton.MouseButton1Click:Connect(function()
        local newState = not isChecked

        -- If onBeforeChange is provided and returns false, don't change
        if onBeforeChange then
            -- onBeforeChange returns true if we should proceed (possibly async with confirmation)
            local shouldProceed = onBeforeChange(newState)
            if not shouldProceed then
                return  -- Change will be handled by confirmation dialog
            end
        end

        updateVisual(newState)
        onChange(newState)
    end)

    return {
        button = checkboxButton,
        setChecked = updateVisual,
    }
end

-- Checkbox references (forward declarations for callbacks)
local studioToFilesCheckbox: {button: TextButton, setChecked: (boolean) -> ()}
local filesToStudioCheckbox: {button: TextButton, setChecked: (boolean) -> ()}

-- Studio → Files checkbox (auto-extract)
studioToFilesCheckbox = createCheckbox(
    syncOptionsSection, 20,
    "Sync Studio changes to files",
    syncSettings.studioToFiles,
    function(newState)
        -- Only show confirmation when enabling
        if newState and isConnected and Config.getProjectDir() ~= "" then
            showConfirmation(
                "Extract to Files",
                "This will extract all Studio instances to your local files. Continue?",
                function()
                    -- User confirmed - update checkbox and run extract
                    studioToFilesCheckbox.setChecked(true)
                    syncSettings.studioToFiles = true
                    ChangeTracker.start()
                    task.spawn(function()
                        extractGame({})
                        changeTracking.studioChanges = 0
                        updateButtonBadges()
                    end)
                end,
                function()
                    -- User cancelled - keep checkbox unchecked
                end
            )
            return false  -- Don't proceed immediately, wait for confirmation
        end
        return true  -- Proceed immediately for unchecking or when not connected
    end,
    function(checked)
        syncSettings.studioToFiles = checked
        if isConnected then
            if checked then
                ChangeTracker.start()
            else
                ChangeTracker.stop()
            end
        end
    end
)

-- Files → Studio checkbox (file watcher)
filesToStudioCheckbox = createCheckbox(
    syncOptionsSection, 44,
    "Sync file changes to Studio",
    syncSettings.filesToStudio,
    function(newState)
        -- Only show confirmation when enabling
        if newState and isConnected and Config.getProjectDir() ~= "" then
            showConfirmation(
                "Sync to Studio",
                "This will sync all files to Studio, overwriting Studio instances. Continue?",
                function()
                    -- User confirmed - update checkbox and run sync
                    filesToStudioCheckbox.setChecked(true)
                    syncSettings.filesToStudio = true
                    task.spawn(function()
                        local projectDir = Config.getProjectDir()
                        local success, result = httpPost("/sync/read-tree", {
                            project_dir = projectDir,
                        })
                        if success and result and result.instances then
                            -- Build set of paths that exist in files
                            local filePaths = {}
                            for _, inst in ipairs(result.instances) do
                                if inst.path and inst.path ~= "" then
                                    filePaths[inst.path] = true
                                end
                            end

                            -- Collect paths from Studio to find what should be deleted
                            local studioPaths = {}
                            for _, serviceName in ipairs({"Workspace", "ReplicatedStorage", "ReplicatedFirst", "ServerScriptService", "ServerStorage", "StarterGui", "StarterPack", "StarterPlayer", "Lighting", "SoundService"}) do
                                local service = game:FindFirstChild(serviceName)
                                if service then
                                    for _, desc in service:GetDescendants() do
                                        local parts = {}
                                        local current = desc
                                        while current and current ~= game do
                                            table.insert(parts, 1, current.Name)
                                            current = current.Parent
                                        end
                                        if #parts > 0 then
                                            local path = table.concat(parts, "/")
                                            studioPaths[path] = desc
                                        end
                                    end
                                end
                            end

                            -- Build operations: updates for existing files, deletes for missing
                            local operations = {}

                            -- Add update operations for files
                            for _, inst in ipairs(result.instances) do
                                if inst.path and inst.path ~= "" then
                                    table.insert(operations, { type = "update", path = inst.path, data = inst })
                                end
                            end

                            -- Add delete operations for instances in Studio but not in files
                            for path, instance in pairs(studioPaths) do
                                if not filePaths[path] then
                                    -- Check that parent path exists in files (don't delete children of deleted parents)
                                    local parentPath = path:match("(.+)/[^/]+$")
                                    if not parentPath or filePaths[parentPath] then
                                        table.insert(operations, { type = "delete", path = path, className = instance.ClassName })
                                        print("[RbxSync Debug] Will delete: " .. path)
                                    end
                                end
                            end

                            if #operations > 0 then
                                syncBatch({ operations = operations })
                            end
                            changeTracking.localChanges = 0
                            updateButtonBadges()
                        end
                    end)
                end,
                function()
                    -- User cancelled - keep checkbox unchecked
                end
            )
            return false  -- Don't proceed immediately, wait for confirmation
        end
        return true  -- Proceed immediately for unchecking or when not connected
    end,
    function(checked)
        syncSettings.filesToStudio = checked
    end
)

-- Auto-connect checkbox
createCheckbox(
    syncOptionsSection, 68,
    "Auto-connect on startup",
    Config.getAutoConnect(),
    nil,  -- No confirmation needed
    function(checked)
        Config.setAutoConnect(checked)
    end
)

-- ============================================================================
-- Panel Button
-- ============================================================================

panelButton.Click:Connect(function()
    widget.Enabled = not widget.Enabled
    panelButton:SetActive(widget.Enabled)

    if widget.Enabled then
        task.spawn(updateInfoDisplay)
        -- Check if we should show connect prompt
        if userDisconnected and not hasShownConnectPrompt and not isConnected then
            task.spawn(function()
                local serverAvailable, _ = httpGet("/health")
                if serverAvailable then
                    showConnectPrompt()
                end
            end)
        end
    end
end)

-- ============================================================================
-- Background Tasks
-- ============================================================================

task.spawn(pollLoop)

-- Connection check loop (respects userDisconnected flag and auto-connect setting)
task.spawn(function()
    while true do
        -- Show connect prompt if server is available but user hasn't connected yet
        if userDisconnected and not hasShownConnectPrompt and not isConnected and widget.Enabled then
            local serverAvailable, _ = httpGet("/health")
            if serverAvailable then
                task.defer(showConnectPrompt)
            end
        end

        -- Only auto-reconnect if user hasn't explicitly disconnected AND auto-connect is enabled
        if not userDisconnected and Config.getAutoConnect() then
            local wasConnected = isConnected
            local success, _ = httpGet("/health")

            -- Update connection state based on actual response
            if success ~= isConnected then
                isConnected = success
                if isConnected then
                    print("Connected")

                    -- Auto-populate or check for path mismatch with VS Code
                    local currentPath = Config.getProjectDir()
                    local infoSuccess, infoData = httpGet("/rbxsync/server-info")

                    if infoSuccess and infoData and infoData.vscode_workspaces and #infoData.vscode_workspaces > 0 then
                        local vscodePath = infoData.vscode_workspaces[1]

                        if currentPath == "" then
                            -- No path set - auto-populate from VS Code
                            Config.setProjectDir(vscodePath)
                            if projectDirInput then
                                projectDirInput.Text = vscodePath
                            end
                            print("[RbxSync] Using VS Code workspace: " .. vscodePath)
                        elseif currentPath ~= vscodePath then
                            -- Path mismatch - check if parent/child relationship
                            local isRelated = string.find(currentPath, vscodePath, 1, true) == 1 or
                                             string.find(vscodePath, currentPath, 1, true) == 1

                            if not isRelated then
                                -- Show mismatch dialog
                                hasShownPathMismatch = true
                                lastKnownVscodePath = vscodePath
                                print("[RbxSync] Path mismatch detected - showing dialog")
                                task.defer(function()
                                    showPathMismatchDialog(
                                        vscodePath,
                                        function() end,  -- onContinue: keep current path
                                        function()  -- onFixPath: switch to VS Code path
                                            Config.setProjectDir(vscodePath)
                                            if projectDirInput then
                                                projectDirInput.Text = vscodePath
                                            end
                                            registerWithServer()
                                            print("[RbxSync] Switched to VS Code workspace: " .. vscodePath)
                                        end,
                                        function() end  -- onCancel: keep current path
                                    )
                                end)
                            end
                        else
                            -- Paths match - record it
                            lastKnownVscodePath = vscodePath
                        end
                    elseif currentPath == "" and infoSuccess and infoData and infoData.cwd and infoData.cwd ~= "" then
                        -- No VS Code, no path - fall back to server CWD
                        Config.setProjectDir(infoData.cwd)
                        if projectDirInput then
                            projectDirInput.Text = infoData.cwd
                        end
                        print("[RbxSync] Using server directory: " .. infoData.cwd)
                    end

                    registerWithServer()
                    -- Start change tracker only if enabled
                    if syncSettings.studioToFiles then
                        ChangeTracker.start()
                    end
                    startConsoleCapture()
                else
                    print("Disconnected")
                    ChangeTracker.stop()
                    stopConsoleCapture()
                end
                updateConnectionUI()
            elseif isConnected then
                -- Still connected - refresh registration to keep heartbeat alive
                registerWithServer()

                -- Check if we were unlinked by VS Code
                local statusSuccess, statusData = httpPost("/rbxsync/check-status", {
                    session_id = SESSION_ID,
                })
                if statusSuccess and statusData and statusData.success and not statusData.linked then
                    -- VS Code unlinked us - disconnect locally (server already has us as unlinked)
                    print("[RbxSync] Unlinked by VS Code - disconnecting")
                    isConnected = false
                    userDisconnected = true
                    hasLoggedLink = false
                    ChangeTracker.stop()
                    stopConsoleCapture()
                    updateConnectionUI()
                end

                -- Check if VS Code just connected with a different path
                if not hasShownPathMismatch then
                    local infoSuccess, infoData = httpGet("/rbxsync/server-info")
                    if infoSuccess and infoData and infoData.vscode_workspaces and #infoData.vscode_workspaces > 0 then
                        local vscodePath = infoData.vscode_workspaces[1]

                        -- Check if this is a NEW VS Code connection
                        if vscodePath ~= lastKnownVscodePath then
                            lastKnownVscodePath = vscodePath
                            local currentPath = Config.getProjectDir()

                            if currentPath ~= "" and currentPath ~= vscodePath then
                                -- Check if parent/child relationship
                                local isRelated = string.find(currentPath, vscodePath, 1, true) == 1 or
                                                 string.find(vscodePath, currentPath, 1, true) == 1

                                if not isRelated then
                                    hasShownPathMismatch = true
                                    print("[RbxSync] VS Code connected with different path - showing dialog")
                                    task.defer(function()
                                        showPathMismatchDialog(
                                            vscodePath,
                                            function() end,  -- onContinue: keep current path
                                            function()  -- onFixPath: switch to VS Code path
                                                Config.setProjectDir(vscodePath)
                                                if projectDirInput then
                                                    projectDirInput.Text = vscodePath
                                                end
                                                registerWithServer()
                                                print("[RbxSync] Switched to VS Code workspace: " .. vscodePath)
                                            end,
                                            function() end  -- onCancel: keep current path
                                        )
                                    end)
                                end
                            end
                        end
                    end
                end
            end
        else
            -- User disconnected - still send heartbeat to stay visible in VS Code
            local success, _ = httpGet("/health")
            if success then
                -- Send heartbeat with empty project_dir to show as unlinked but visible
                httpPost("/rbxsync/register", {
                    place_id = game.PlaceId,
                    place_name = getPlaceName(),
                    project_dir = "",  -- Empty = unlinked
                    session_id = SESSION_ID,
                })
            end
            if not success and isConnected then
                isConnected = false
                updateConnectionUI()
            end
        end
        task.wait(5)
    end
end)

-- Info and change count refresh loop
task.spawn(function()
    -- Initial update
    task.wait(1)
    updateInfoDisplay()

    while true do
        if widget.Enabled then
            updateInfoDisplay()
            -- Fetch pending local changes for button badge
            if isConnected then
                task.spawn(fetchLocalChanges)
            end
        end
        task.wait(5)
    end
end)

-- Check for updates on startup
task.spawn(checkForUpdates)

-- Cleanup when plugin unloads (Studio closes or plugin reloads) - unregister from server
plugin.Unloading:Connect(function()
    unregisterWithServer()
end)
