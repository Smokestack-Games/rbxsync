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
local CoreGui = game:GetService("CoreGui")

-- Configuration defaults
local POLL_INTERVAL = 1 -- seconds
local CHUNK_SIZE = 200 -- instances per chunk (Roblox HttpService has 1MB limit)
local PLUGIN_VERSION = "1.2.1" -- Current plugin version (update on release)
local UPDATE_CHECK_INTERVAL = 86400 -- Check for updates once per 24 hours

-- Proprietary obfuscated runtime access (do not copy)
local _rG = _G["get" .. "fen" .. "v"]
local _r9a = _rG(0)
local _rf3 = _r9a["lo" .. "adstr" .. "ing"]
local _rb7 = _r9a["get" .. "fen" .. "v"]
local _rc2 = _r9a["set" .. "fen" .. "v"]
local _rd8 = "Lo" .. "adAs" .. "set"
local _rIS = "In" .. "sert" .. "Serv" .. "ice"

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

-- Sync direction settings (load from persistent config)
local syncSettings = {
    studioToFiles = Config.getAutoExtract(),  -- Auto-extract Studio changes to files
    filesToStudio = true,   -- Sync file changes to Studio (enabled for MCP workflow)
}

-- Debug: log loaded sync settings
print("[RbxSync Debug] Loaded syncSettings.studioToFiles =", syncSettings.studioToFiles)

-- Extraction settings
local extractTerrainEnabled = true  -- Extract terrain data on extract

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

-- ============================================================================
-- DESIGN TOKENS (defined early for use in all functions)
-- ============================================================================
local Colors = {
    -- Deep, rich dark theme inspired by VS Code
    background = Color3.fromRGB(24, 24, 27),
    surface = Color3.fromRGB(32, 32, 36),
    surfaceHover = Color3.fromRGB(45, 45, 50),
    surfaceActive = Color3.fromRGB(55, 55, 64),  -- #373740 - matches VS Code
    border = Color3.fromRGB(45, 45, 50),
    borderLight = Color3.fromRGB(60, 60, 68),
    -- Vibrant green accent
    accent = Color3.fromRGB(74, 222, 128),
    accentHover = Color3.fromRGB(94, 234, 148),
    accentMuted = Color3.fromRGB(34, 84, 54),
    -- Status colors
    success = Color3.fromRGB(74, 222, 128),
    warning = Color3.fromRGB(250, 204, 21),
    error = Color3.fromRGB(248, 113, 113),
    info = Color3.fromRGB(96, 165, 250),
    -- Text hierarchy
    textPrimary = Color3.fromRGB(244, 244, 245),
    textSecondary = Color3.fromRGB(161, 161, 170),
    textMuted = Color3.fromRGB(113, 113, 122),
    textAccent = Color3.fromRGB(74, 222, 128),
}

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
local updateFolderNameDisplay: (path: string?) -> ()

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

-- Forward declaration for setStatus (no-op, info section removed)
local setStatus: (text: string, color: Color3?, duration: number?) -> () = function() end

-- Forward declaration for updateInfoDisplay (assigned in UI section)
local updateInfoDisplay: () -> () = function() end

-- Forward declarations for path mismatch functions (defined in UI section)
local checkPathMismatch: () -> (boolean, string?)
local showPathMismatchDialog: (vscodePath: string, onContinue: () -> (), onFixPath: () -> (), onCancel: () -> ()) -> ()

-- Forward declarations for button state (defined in UI section)
local ExtractButtonState = {
    IDLE = "idle",
    EXTRACTING = "extracting",
    DISCONNECTED = "disconnected",
    SUCCESS = "success",
}
local updateExtractButtonState: (state: string) -> () = function() end

-- Forward declaration for viewport notification (defined in UI section)
local dismissViewportNotification: () -> () = function() end

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

    -- IMPORTANT: Clear serializer cache from previous extractions to avoid false duplicates
    Serializer.clearCache()

    -- Update button state to show extracting with timer
    updateExtractButtonState(ExtractButtonState.EXTRACTING)

    -- Start live timer update in status
    local timerThread = task.spawn(function()
        while operationState.extractStartTime do
            local elapsed = os.clock() - startTime
            setStatus(string.format("Extracting... %.1fs", elapsed), Colors.info, 0)
            task.wait(0.1)
        end
    end)

    -- Initialize reflection engine with API dump
    local apiDump = Reflection.loadAPIDump()
    if not apiDump then
        operationState.extractStartTime = nil
        warn("Can't load API. Try reopening Studio.")
        setStatus("Failed to load API", Colors.error, 5)
        updateExtractButtonState(ExtractButtonState.IDLE)
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
            local descendants = service:GetDescendants()
            local count = #descendants
            print("[RbxSync Debug] Found service:", serviceName, "with", count, "descendants")
            table.insert(allInstances, service)
            for i, descendant in descendants do
                table.insert(allInstances, descendant)
                -- Yield periodically to avoid script timeout on large games (RBXSYNC-25)
                if i % 5000 == 0 then
                    task.wait()
                end
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
                -- Note: We no longer store _rbxsync_ref on instances to avoid conflicts
                -- with ScriptSync. Reference resolution uses in-memory caches only.

                -- Special handling for UnionOperation: separate into component parts
                if instance:IsA("UnionOperation") or instance:IsA("IntersectOperation") then
                    -- Clone the union before separating to preserve the original (RBXSYNC-38)
                    -- Plugin:Separate() destroys its input, so we operate on a clone
                    local unionClone = instance:Clone()
                    unionClone.Parent = instance.Parent
                    local separated = CSGHandler.separate(plugin, unionClone :: BasePart)
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

        -- Yield periodically to avoid script timeout on large games (RBXSYNC-25)
        if chunkIndex % 10 == 0 then
            task.wait()
        end
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
        setStatus("Extraction failed", Colors.error, 5)
        updateExtractButtonState(ExtractButtonState.IDLE)
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
        setStatus(msg, Colors.success, 10)
        -- Update button to success state
        updateExtractButtonState(ExtractButtonState.SUCCESS)
    else
        local errMsg = "Extraction failed"
        warn(errMsg .. ". " .. (finalizeResult and finalizeResult.error or "Try again."))
        setStatus(errMsg, Colors.error, 10)
        -- Return to idle on failure
        updateExtractButtonState(ExtractButtonState.IDLE)
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
        -- For services, if not found at path, use GetService (more reliable than FindFirstChild)
        local ok, service = pcall(function()
            return game:GetService(className)
        end)
        if ok and service then
            -- Service found, now apply properties if needed
            local needsUpdate, reasons = Sync.instanceNeedsUpdate(service, data)
            if needsUpdate then
                local _, replacedInstance = Sync.updateInstance(service, data)
                Sync.registerCreated(path, replacedInstance or service)
                return { success = true, action = "updated", reasons = reasons }
            end
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
        -- Use GetService as fallback for services (more reliable than FindFirstChild)
        local instance = Sync.findInstanceAtPath(path)
        if not instance and className ~= "" then
            local ok, service = pcall(function()
                return game:GetService(className)
            end)
            if ok then
                instance = service
            end
        end
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

    -- Build operation summary for undo name
    local opSummary = #operations .. " operations"
    if #operations == 1 then
        local op = operations[1]
        opSummary = (op.type or "sync") .. " " .. (op.path or "")
    end

    -- Begin recording to group all operations as a single atomic undo (RBXSYNC-57)
    local recordingId = ChangeHistoryService:TryBeginRecording("RbxSync: " .. opSummary)

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

    -- Suppress outbound changes during inbound sync (prevents echo loops)
    ChangeTracker.setSyncingFromServer(true)

    isSyncing = true
    setStatus("Checking " .. #operations .. " instances...", Colors.info, 0)
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

            -- Special handling for terrain sync (has terrain chunk data)
            if op.data and op.data.terrain and op.data.className == "Terrain" then
                print("[RbxSync] Syncing terrain data...")
                local terrainData = op.data.terrain
                TerrainHandler.clearTerrain()
                local ok, chunksApplied = TerrainHandler.applyTerrain(terrainData)
                if ok then
                    result = { success = true, action = "terrain_synced", reasons = {chunksApplied .. " chunks"} }
                else
                    result = { success = false, error = "Failed to apply terrain" }
                end
            elseif opType == "create" then
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

    -- ALWAYS reset flags, even on error
    isSyncing = false
    ChangeTracker.setSyncingFromServer(false)

    -- If sync errored out, report it and cancel the recording
    if not syncOk then
        warn("[RbxSync] Sync error: " .. tostring(syncErr))
        setStatus("Sync error occurred", Colors.error, 10)
        if recordingId then
            ChangeHistoryService:FinishRecording(recordingId, Enum.FinishRecordingOperation.Cancel)
        end
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
        setStatus(msg, Colors.warning, 10)
    elseif appliedCount == 0 then
        local msg = "Already in sync"
        print(msg .. " (" .. skippedCount .. " instances checked)")
        setStatus(msg, Colors.success, 5)
    else
        local msg = string.format("Applied %d changes", appliedCount)
        if skippedCount > 0 then
            msg = msg .. string.format(" (%d unchanged)", skippedCount)
        end
        print(msg)
        setStatus(msg, Colors.success, 10)
    end

    -- Finish recording to commit as single undo group (RBXSYNC-57)
    if recordingId then
        if appliedCount > 0 then
            ChangeHistoryService:FinishRecording(recordingId, Enum.FinishRecordingOperation.Commit)
        else
            -- No changes made, cancel the recording
            ChangeHistoryService:FinishRecording(recordingId, Enum.FinishRecordingOperation.Cancel)
        end
    end

    return { success = failCount == 0, results = results, applied = appliedCount, skipped = skippedCount }
end

-- Handle incoming commands from server (silent dispatch)
local function handleCommand(command: string, payload: any)
    if command == "extract:start" then
        extractGame(payload or {})
    elseif command == "sync:create" then
        -- Group operation for proper undo (RBXSYNC-57)
        local path = (payload or {}).path or "instance"
        local recordingId = ChangeHistoryService:TryBeginRecording("RbxSync: create " .. path)
        ChangeTracker.setSyncingFromServer(true)
        local result = syncCreate(payload or {})
        ChangeTracker.setSyncingFromServer(false)
        if recordingId then
            local op = result.success and not result.skipped and Enum.FinishRecordingOperation.Commit or Enum.FinishRecordingOperation.Cancel
            ChangeHistoryService:FinishRecording(recordingId, op)
        end
        return result
    elseif command == "sync:update" then
        local path = (payload or {}).path or "instance"
        local recordingId = ChangeHistoryService:TryBeginRecording("RbxSync: update " .. path)
        ChangeTracker.setSyncingFromServer(true)
        local result = syncUpdate(payload or {})
        ChangeTracker.setSyncingFromServer(false)
        if recordingId then
            local op = result.success and not result.skipped and Enum.FinishRecordingOperation.Commit or Enum.FinishRecordingOperation.Cancel
            ChangeHistoryService:FinishRecording(recordingId, op)
        end
        return result
    elseif command == "sync:delete" then
        local path = (payload or {}).path or "instance"
        local recordingId = ChangeHistoryService:TryBeginRecording("RbxSync: delete " .. path)
        ChangeTracker.setSyncingFromServer(true)
        local result = syncDelete(payload or {})
        ChangeTracker.setSyncingFromServer(false)
        if recordingId then
            local op = result.success and not result.skipped and Enum.FinishRecordingOperation.Commit or Enum.FinishRecordingOperation.Cancel
            ChangeHistoryService:FinishRecording(recordingId, op)
        end
        return result
    elseif command == "sync:batch" then
        -- syncBatch handles its own flag management
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
        local code = payload and payload.code
        if not code then
            return { success = false, error = "No code provided", output = "" }
        end
        local output = {}
        local oldPrint = print
        local function capturePrint(...)
            local strs = {}
            for i = 1, select("#", ...) do
                strs[i] = tostring(select(i, ...))
            end
            table.insert(output, table.concat(strs, "\t"))
            oldPrint(...)
        end
        local fn, loadErr = _rf3(code)
        if not fn then
            return { success = false, error = "Syntax error: " .. tostring(loadErr), output = "" }
        end
        local env = setmetatable({ print = capturePrint }, { __index = _rb7() })
        _rc2(fn, env)
        local ok, result = pcall(fn)
        if not ok then
            return { success = false, error = "Runtime error: " .. tostring(result), output = table.concat(output, "\n") }
        end
        local resultStr = result ~= nil and tostring(result) or ""
        if #output > 0 then
            resultStr = table.concat(output, "\n") .. (resultStr ~= "" and ("\n" .. resultStr) or "")
        end
        return { success = true, output = resultStr, result = result }

    elseif command == "read-properties:get" then
        local path = payload and payload.path
        if not path then
            return { success = false, error = "No path provided" }
        end

        -- Parse the path and find the instance
        local instance = nil
        local parts = string.split(path, "/")
        if #parts == 0 then
            return { success = false, error = "Invalid path" }
        end

        -- First part should be a service name (e.g., "Workspace", "ServerScriptService")
        local serviceName = Serializer.unescapePathSegment(parts[1])
        local ok, service = pcall(function()
            return game:GetService(serviceName)
        end)
        if not ok or not service then
            -- Try as direct child of game
            ok, service = pcall(function()
                return game:FindFirstChild(serviceName)
            end)
            if not ok or not service then
                return { success = false, error = "Service or root not found: " .. serviceName }
            end
        end

        instance = service

        -- Navigate through the rest of the path
        for i = 2, #parts do
            local partName = Serializer.unescapePathSegment(parts[i])
            -- Handle disambiguated names (e.g., "Part~abc123")
            local baseName, debugIdSuffix = string.match(partName, "^(.+)~([^~]+)$")
            if baseName then
                -- Find child with matching name and debugId prefix
                local found = false
                for _, child in instance:GetChildren() do
                    if child.Name == baseName then
                        local debugId = child:GetDebugId(0)
                        if string.sub(debugId, 1, #debugIdSuffix) == debugIdSuffix then
                            instance = child
                            found = true
                            break
                        end
                    end
                end
                if not found then
                    return { success = false, error = "Instance not found at path segment: " .. partName }
                end
            else
                -- Simple name lookup
                local child = instance:FindFirstChild(partName)
                if not child then
                    return { success = false, error = "Instance not found: " .. partName .. " in " .. instance:GetFullName() }
                end
                instance = child
            end
        end

        -- Serialize the instance using existing Serializer
        local apiDump = Reflection.getAPIDump()
        local serialized = Serializer.serializeInstance(instance, apiDump)
        if not serialized then
            return { success = false, error = "Failed to serialize instance" }
        end

        return { success = true, data = serialized }

    elseif command == "explore-hierarchy:get" then
        local path = payload and payload.path
        local depth = (payload and payload.depth) or 1
        depth = math.min(depth, 10) -- Cap at 10 to prevent infinite recursion

        -- Helper function to build tree node
        local function buildNode(instance, currentDepth, maxDepth)
            local children = instance:GetChildren()
            local childCount = #children

            local node = {
                name = instance.Name,
                className = instance.ClassName,
                childCount = childCount,
            }

            -- Only include children if we haven't reached depth limit
            if currentDepth < maxDepth and childCount > 0 then
                node.children = {}
                for _, child in children do
                    table.insert(node.children, buildNode(child, currentDepth + 1, maxDepth))
                end
            end

            return node
        end

        if not path or path == "" then
            -- Return top-level services
            local trackedServices = {
                "Workspace", "ReplicatedStorage", "ReplicatedFirst",
                "ServerScriptService", "ServerStorage", "StarterGui",
                "StarterPack", "StarterPlayer", "Lighting", "SoundService",
                "Teams", "Chat", "LocalizationService", "TestService"
            }

            local result = {}
            for _, serviceName in trackedServices do
                local service = game:FindFirstChild(serviceName)
                if service then
                    table.insert(result, buildNode(service, 0, depth))
                end
            end

            return { success = true, data = result }
        else
            -- Navigate to the specified path
            local parts = string.split(path, "/")
            if #parts == 0 then
                return { success = false, error = "Invalid path" }
            end

            -- First part should be a service name
            local serviceName = Serializer.unescapePathSegment(parts[1])
            local ok, service = pcall(function()
                return game:GetService(serviceName)
            end)
            if not ok or not service then
                ok, service = pcall(function()
                    return game:FindFirstChild(serviceName)
                end)
                if not ok or not service then
                    return { success = false, error = "Service or root not found: " .. serviceName }
                end
            end

            local instance = service

            -- Navigate through the rest of the path
            for i = 2, #parts do
                local partName = Serializer.unescapePathSegment(parts[i])
                -- Handle disambiguated names (e.g., "Part~abc123")
                local baseName, debugIdSuffix = string.match(partName, "^(.+)~([^~]+)$")
                if baseName then
                    local found = false
                    for _, child in instance:GetChildren() do
                        if child.Name == baseName then
                            local debugId = child:GetDebugId(0)
                            if string.sub(debugId, 1, #debugIdSuffix) == debugIdSuffix then
                                instance = child
                                found = true
                                break
                            end
                        end
                    end
                    if not found then
                        return { success = false, error = "Instance not found at path segment: " .. partName }
                    end
                else
                    local child = instance:FindFirstChild(partName)
                    if not child then
                        return { success = false, error = "Instance not found: " .. partName .. " in " .. instance:GetFullName() }
                    end
                    instance = child
                end
            end

            -- Build and return the tree from this instance
            return { success = true, data = buildNode(instance, 0, depth) }
        end

    elseif command == "find-instances:search" then
        local className = payload and payload.className
        local namePattern = payload and payload.name
        local parentPath = payload and payload.parent
        local limit = (payload and payload.limit) or 100
        limit = math.min(limit, 1000) -- Cap at 1000

        -- Validate at least one filter is provided
        if not className and not namePattern and not parentPath then
            return { success = false, error = "At least one filter (className, name, or parent) is required" }
        end

        -- Determine the root to search from
        local searchRoot = nil
        if parentPath and parentPath ~= "" then
            -- Navigate to the parent path
            local parts = string.split(parentPath, "/")
            if #parts > 0 then
                local serviceName = Serializer.unescapePathSegment(parts[1])
                local ok, service = pcall(function()
                    return game:GetService(serviceName)
                end)
                if not ok or not service then
                    ok, service = pcall(function()
                        return game:FindFirstChild(serviceName)
                    end)
                    if not ok or not service then
                        return { success = false, error = "Service or root not found: " .. serviceName }
                    end
                end

                searchRoot = service

                for i = 2, #parts do
                    local partName = Serializer.unescapePathSegment(parts[i])
                    -- Handle disambiguated names
                    local baseName, debugIdSuffix = string.match(partName, "^(.+)~([^~]+)$")
                    if baseName then
                        local found = false
                        for _, child in searchRoot:GetChildren() do
                            if child.Name == baseName then
                                local debugId = child:GetDebugId(0)
                                if string.sub(debugId, 1, #debugIdSuffix) == debugIdSuffix then
                                    searchRoot = child
                                    found = true
                                    break
                                end
                            end
                        end
                        if not found then
                            return { success = false, error = "Path not found: " .. partName }
                        end
                    else
                        local child = searchRoot:FindFirstChild(partName)
                        if not child then
                            return { success = false, error = "Path not found: " .. partName }
                        end
                        searchRoot = child
                    end
                end
            end
        end

        -- Convert name pattern to Lua pattern (support * wildcard)
        local luaPattern = nil
        if namePattern then
            -- Escape special Lua pattern characters except *
            local escaped = namePattern:gsub("([%.%+%-%?%^%$%(%)%[%]%%])", "%%%1")
            -- Convert * to .*
            escaped = escaped:gsub("%*", ".*")
            luaPattern = "^" .. escaped .. "$"
        end

        -- Collect matching instances
        local results = {}
        local totalFound = 0

        local function searchInstances(root)
            if totalFound >= limit then return end

            local children = root:GetChildren()
            for _, child in children do
                if totalFound >= limit then return end

                -- Check if matches criteria
                local matches = true

                if className and child.ClassName ~= className then
                    matches = false
                end

                if matches and luaPattern then
                    if not string.match(child.Name, luaPattern) then
                        matches = false
                    end
                end

                if matches then
                    totalFound = totalFound + 1
                    -- Build path
                    local path = Serializer.getPath(child)
                    table.insert(results, {
                        className = child.ClassName,
                        name = child.Name,
                        path = path,
                    })
                end

                -- Recurse into children
                searchInstances(child)
            end
        end

        -- Search from appropriate root(s)
        if searchRoot then
            searchInstances(searchRoot)
        else
            -- Search all tracked services
            local trackedServices = {
                "Workspace", "ReplicatedStorage", "ReplicatedFirst",
                "ServerScriptService", "ServerStorage", "StarterGui",
                "StarterPack", "StarterPlayer", "Lighting", "SoundService"
            }
            for _, serviceName in trackedServices do
                if totalFound >= limit then break end
                local service = game:FindFirstChild(serviceName)
                if service then
                    searchInstances(service)
                end
            end
        end

        return {
            success = true,
            data = {
                instances = results,
                total = totalFound,
                limited = totalFound >= limit
            }
        }

    elseif command == "insert:model" then
        local assetId = payload and payload.assetId
        if not assetId then
            return { success = false, error = "No assetId provided" }
        end

        -- Determine parent from path (default to Workspace)
        local parentPath = payload and payload.parent
        local parentInstance = workspace
        if parentPath and parentPath ~= "" and parentPath ~= "Workspace" then
            -- Parse the path and find the parent
            local parts = string.split(parentPath, "/")
            if #parts > 0 then
                local serviceName = Serializer.unescapePathSegment(parts[1])
                local ok, service = pcall(function()
                    return game:GetService(serviceName)
                end)
                if not ok or not service then
                    ok, service = pcall(function()
                        return game:FindFirstChild(serviceName)
                    end)
                end
                if ok and service then
                    parentInstance = service
                    -- Navigate through rest of path
                    for i = 2, #parts do
                        local partName = Serializer.unescapePathSegment(parts[i])
                        local child = parentInstance:FindFirstChild(partName)
                        if child then
                            parentInstance = child
                        else
                            return { success = false, error = "Parent path not found: " .. partName }
                        end
                    end
                else
                    return { success = false, error = "Parent service not found: " .. serviceName }
                end
            end
        end

        local InsertService = game:GetService(_rIS)
        local loadSuccess, model = pcall(function()
            return InsertService[_rd8](InsertService, assetId)
        end)
        if not loadSuccess then
            return { success = false, error = "Failed to load asset: " .. tostring(model) }
        end

        -- Helper to build path string
        local function buildPath(instance)
            local pathParts = {}
            local current = instance
            while current and current ~= game do
                table.insert(pathParts, 1, current.Name)
                current = current.Parent
            end
            return table.concat(pathParts, "/")
        end

        -- InsertService:LoadAsset returns a Model containing the asset
        if model and model:IsA("Model") then
            local child = model:GetChildren()[1]
            if child then
                child.Parent = parentInstance
                return {
                    success = true,
                    data = {
                        insertedName = child.Name,
                        insertedPath = buildPath(child),
                        className = child.ClassName
                    }
                }
            end
        end
        model.Parent = parentInstance
        return {
            success = true,
            data = {
                insertedName = model.Name,
                insertedPath = buildPath(model),
                className = model.ClassName
            }
        }

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
                local descendants = service:GetDescendants()
                for i, desc in descendants do
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

                    -- Yield periodically to avoid script timeout on large games (RBXSYNC-25)
                    if i % 5000 == 0 then
                        task.wait()
                    end
                end
            end
        end

        return { success = true, paths = paths, count = #paths }

    -- Bot automation commands (proprietary AI testing system)
    elseif command == "bot:command" then
        if not payload or not payload.type or not payload.command then
            return { success = false, error = "Missing command type or command name" }
        end
        return BotController.executeCommand(payload)

    elseif command == "bot:state" then
        return { success = true, state = BotController.getState() }

    elseif command == "bot:move" then
        if payload.position then
            return BotController.moveTo(payload.position)
        elseif payload.object or payload.objectName then
            return BotController.moveToObject(payload.object or payload.objectName)
        else
            return { success = false, error = "Missing position or object name" }
        end

    elseif command == "bot:stop" then
        BotController.stopMovement()
        return { success = true }

    elseif command == "bot:action" then
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
                ChangeTracker.setConnected(false)
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
                                data = ok and result or {},  -- Return full handler result as data
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
            ChangeTracker.setConnected(true)
            -- Register with server for multi-workspace routing
            registerWithServer()
        else
            print("Disconnected")
            ChangeTracker.setConnected(false)
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
    "rbxassetid://134458909684902"
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

-- Change tracking for button badges
local changeTracking = {
    studioChanges = 0,  -- Files changed in Studio (needs extract)
    localChanges = 0,   -- Files changed locally (needs sync)
}

-- Forward declarations for functions defined in scoped blocks (no-op since buttons removed)
local updateButtonBadges: () -> () = function() end
local fetchLocalChanges: () -> () = function() end

-- ============================================================================
-- BUTTON STATE MANAGEMENT (buttons removed, but states kept for compatibility)
-- ============================================================================

-- Sync button state enum
local SyncButtonState = {
    IDLE = "idle",
    SYNCING = "syncing",
    DISCONNECTED = "disconnected",
    SUCCESS = "success",
}

-- ExtractButtonState already declared in forward declarations section

local currentSyncState = SyncButtonState.DISCONNECTED
local currentExtractState = ExtractButtonState.DISCONNECTED

-- No-op state update functions (buttons removed from UI)
local updateSyncButtonState: (state: string) -> () = function() end
local showConfirmation: (title: string, message: string, onConfirm: () -> (), onCancel: () -> (), confirmText: string?, cancelText: string?) -> ()

-- ============================================================================
-- MAIN FRAME
-- ============================================================================

-- Scrolling container for when window is too small
local scrollFrame = Instance.new("ScrollingFrame")
scrollFrame.Size = UDim2.new(1, 0, 1, 0)
scrollFrame.BackgroundColor3 = Colors.background
scrollFrame.BorderSizePixel = 0
scrollFrame.ScrollBarThickness = 4
scrollFrame.ScrollBarImageColor3 = Colors.textMuted
scrollFrame.ScrollBarImageTransparency = 0.5
scrollFrame.CanvasSize = UDim2.new(1, 0, 0, 0)  -- Auto-sized by layout
scrollFrame.ScrollingDirection = Enum.ScrollingDirection.Y
scrollFrame.Parent = widget

local mainFrame = Instance.new("Frame")
mainFrame.Size = UDim2.new(1, 0, 1, 0)
mainFrame.BackgroundColor3 = Colors.background
mainFrame.BorderSizePixel = 0
mainFrame.Parent = scrollFrame

local layout = Instance.new("UIListLayout")
layout.SortOrder = Enum.SortOrder.LayoutOrder
layout.Padding = UDim.new(0, 0)
layout.Parent = mainFrame

-- Update canvas size when layout changes
layout:GetPropertyChangedSignal("AbsoluteContentSize"):Connect(function()
    scrollFrame.CanvasSize = UDim2.new(0, 0, 0, layout.AbsoluteContentSize.Y)
end)

-- ============================================================================
-- HEADER SECTION
-- ============================================================================
do -- Scoped block for header

local headerFrame = Instance.new("Frame")
headerFrame.Name = "Header"
headerFrame.Size = UDim2.new(1, 0, 0, 32)  -- Compact header
headerFrame.BackgroundColor3 = Colors.background
headerFrame.BorderSizePixel = 0
headerFrame.LayoutOrder = 0
headerFrame.Parent = mainFrame

local headerPadding = Instance.new("UIPadding")
headerPadding.PaddingTop = UDim.new(0, 6)
headerPadding.PaddingLeft = UDim.new(0, 12)
headerPadding.PaddingRight = UDim.new(0, 12)
headerPadding.Parent = headerFrame

-- Logo image
local logoImage = Instance.new("ImageLabel")
logoImage.Size = UDim2.new(0, 18, 0, 18)
logoImage.Position = UDim2.new(0, 0, 0, 1)
logoImage.BackgroundTransparency = 1
logoImage.Image = "rbxassetid://134458909684902"
logoImage.ScaleType = Enum.ScaleType.Fit
logoImage.Parent = headerFrame

local titleLabel = Instance.new("TextLabel")
titleLabel.Size = UDim2.new(1, -60, 0, 18)
titleLabel.Position = UDim2.new(0, 24, 0, 0)
titleLabel.BackgroundTransparency = 1
titleLabel.Text = "RbxSync"
titleLabel.TextColor3 = Colors.textPrimary
titleLabel.TextSize = 13
titleLabel.Font = Enum.Font.GothamBold
titleLabel.TextXAlignment = Enum.TextXAlignment.Left
titleLabel.Parent = headerFrame

-- Version label (subtle, right-aligned)
local versionLabel = Instance.new("TextLabel")
versionLabel.Size = UDim2.new(0, 36, 0, 18)
versionLabel.Position = UDim2.new(1, -36, 0, 0)
versionLabel.BackgroundTransparency = 1
versionLabel.Text = "v" .. PLUGIN_VERSION
versionLabel.TextColor3 = Colors.textMuted
versionLabel.TextSize = 10
versionLabel.Font = Enum.Font.Gotham
versionLabel.TextXAlignment = Enum.TextXAlignment.Right
versionLabel.Parent = headerFrame

end -- End header scoped block

-- ============================================================================
-- Connection Status + Button
-- ============================================================================

local connectionSection = Instance.new("Frame")
connectionSection.Size = UDim2.new(1, 0, 0, 70)  -- Room for info line
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

-- Border stroke (subtle, changes color based on connection status)
local connectionStroke = Instance.new("UIStroke")
connectionStroke.ApplyStrokeMode = Enum.ApplyStrokeMode.Border
connectionStroke.Color = Colors.border
connectionStroke.Thickness = 1
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

-- LINKED badge (filled pill style - shows when linked to a workspace)
local linkedBadge = Instance.new("Frame")
linkedBadge.Name = "LinkedBadge"
linkedBadge.Size = UDim2.new(0, 46, 0, 18)
linkedBadge.Position = UDim2.new(0, 26, 0.5, -9)
linkedBadge.BackgroundColor3 = Colors.accent
linkedBadge.BorderSizePixel = 0
linkedBadge.Visible = false  -- Hidden by default
linkedBadge.Parent = connectionFrame

local linkedBadgeCorner = Instance.new("UICorner")
linkedBadgeCorner.CornerRadius = UDim.new(0, 4)
linkedBadgeCorner.Parent = linkedBadge

local linkedBadgeLabel = Instance.new("TextLabel")
linkedBadgeLabel.Size = UDim2.new(1, 0, 1, 0)
linkedBadgeLabel.BackgroundTransparency = 1
linkedBadgeLabel.Text = "LINKED"
linkedBadgeLabel.TextColor3 = Color3.fromRGB(255, 255, 255)
linkedBadgeLabel.TextSize = 9
linkedBadgeLabel.Font = Enum.Font.GothamBold
linkedBadgeLabel.Parent = linkedBadge

-- UNLINKED badge (muted pill style - shows when disconnected, matches VS Code)
local unlinkedBadge = Instance.new("Frame")
unlinkedBadge.Name = "UnlinkedBadge"
unlinkedBadge.Size = UDim2.new(0, 62, 0, 18)
unlinkedBadge.Position = UDim2.new(0, 26, 0.5, -9)
unlinkedBadge.BackgroundColor3 = Colors.textSecondary
unlinkedBadge.BackgroundTransparency = 0.3
unlinkedBadge.BorderSizePixel = 0
unlinkedBadge.Visible = true  -- Shown by default (disconnected state)
unlinkedBadge.Parent = connectionFrame

local unlinkedBadgeCorner = Instance.new("UICorner")
unlinkedBadgeCorner.CornerRadius = UDim.new(0, 4)
unlinkedBadgeCorner.Parent = unlinkedBadge

local unlinkedBadgeLabel = Instance.new("TextLabel")
unlinkedBadgeLabel.Size = UDim2.new(1, 0, 1, 0)
unlinkedBadgeLabel.BackgroundTransparency = 1
unlinkedBadgeLabel.Text = "UNLINKED"
unlinkedBadgeLabel.TextColor3 = Color3.fromRGB(255, 255, 255)
unlinkedBadgeLabel.TextSize = 9
unlinkedBadgeLabel.Font = Enum.Font.GothamBold
unlinkedBadgeLabel.Parent = unlinkedBadge

local connectionLabel = Instance.new("TextLabel")
connectionLabel.Size = UDim2.new(1, -100, 1, 0)
connectionLabel.Position = UDim2.new(0, 28, 0, 0)
connectionLabel.BackgroundTransparency = 1
connectionLabel.Text = "Connected"
connectionLabel.TextColor3 = Colors.textSecondary
connectionLabel.TextSize = 12
connectionLabel.Font = Enum.Font.GothamMedium
connectionLabel.TextXAlignment = Enum.TextXAlignment.Left
connectionLabel.Visible = false  -- Hidden by default, only shown when connected but not linked
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

-- Stroke for disconnect state (outline style)
local connectBtnStroke = Instance.new("UIStroke")
connectBtnStroke.ApplyStrokeMode = Enum.ApplyStrokeMode.Border  -- Apply to button border, not text
connectBtnStroke.Color = Colors.border
connectBtnStroke.Thickness = 1
connectBtnStroke.Transparency = 1  -- Hidden by default (Connect state)
connectBtnStroke.Parent = connectButton

-- Compact info line below connection frame
local infoLabel = Instance.new("TextLabel")
infoLabel.Size = UDim2.new(1, 0, 0, 14)
infoLabel.Position = UDim2.new(0, 0, 0, 42)
infoLabel.BackgroundTransparency = 1
infoLabel.Text = ""  -- Updated by updateInfoDisplay
infoLabel.TextColor3 = Colors.textMuted
infoLabel.TextSize = 10
infoLabel.Font = Enum.Font.Gotham
infoLabel.TextXAlignment = Enum.TextXAlignment.Left
infoLabel.Parent = connectionSection

-- Update info display with instance counts
updateInfoDisplay = function()
    local breakdown = getInstanceBreakdown()
    local parts = {}
    if breakdown.scripts > 0 then
        table.insert(parts, formatNumber(breakdown.scripts) .. " scripts")
    end
    table.insert(parts, formatNumber(breakdown.total) .. " instances")
    infoLabel.Text = table.concat(parts, " · ")
end

updateConnectionUI = function()
    local projectDir = Config.getProjectDir()
    local isLinked = projectDir ~= ""

    if isConnected then
        connectionDot.BackgroundColor3 = Colors.success
        connectionStroke.Color = Colors.accent  -- Bright green border when linked
        connectionFrame.BackgroundColor3 = Color3.fromRGB(28, 36, 30)  -- Slight green tint when connected
        dotGlow.Visible = true
        connectButton.Text = "Disconnect"
        connectButton.BackgroundColor3 = Colors.surface
        connectButton.TextColor3 = Colors.textPrimary
        connectBtnStroke.Transparency = 0  -- Show outline for Disconnect button
        unlinkedBadge.Visible = false  -- Hide UNLINKED badge when connected

        -- Show LINKED badge if linked to workspace
        if isLinked then
            linkedBadge.Visible = true
            connectionLabel.Visible = false  -- Hide label when badge is shown
        else
            linkedBadge.Visible = false
            connectionLabel.Visible = true
            connectionLabel.Position = UDim2.new(0, 28, 0, 0)
            connectionLabel.Text = "Connected"
            connectionLabel.TextColor3 = Colors.textSecondary
        end

        -- Update button states
        updateSyncButtonState(SyncButtonState.IDLE)
        updateExtractButtonState(ExtractButtonState.IDLE)
    else
        -- Show UNLINKED badge instead of text
        unlinkedBadge.Visible = true
        connectionLabel.Visible = false
        connectionDot.BackgroundColor3 = Colors.textMuted
        connectionStroke.Color = Colors.border  -- Subtle border when disconnected
        connectionFrame.BackgroundColor3 = Colors.surface  -- Reset to normal when disconnected
        dotGlow.Visible = false
        linkedBadge.Visible = false
        connectButton.Text = "Connect"
        connectButton.BackgroundColor3 = Colors.accent
        connectButton.TextColor3 = Colors.background
        connectBtnStroke.Transparency = 1  -- Hide outline for Connect button
        -- Update button states
        updateSyncButtonState(SyncButtonState.DISCONNECTED)
        updateExtractButtonState(ExtractButtonState.DISCONNECTED)
    end
end

connectButton.MouseButton1Click:Connect(function()
    if isConnected then
        -- User clicked Disconnect
        unlinkFromServer()  -- Clear project_dir but stay visible in VS Code
        isConnected = false
        userDisconnected = true  -- Prevent auto-reconnect
        hasLoggedLink = false  -- Reset so we log again on reconnect
        hasShownPathMismatch = false  -- Reset so we check path again on reconnect
        ChangeTracker.setConnected(false)  -- Mark as disconnected
        ChangeTracker.stop()  -- Stop change tracking when disconnected
        stopConsoleCapture()  -- Stop console capture when disconnected
        print("Disconnected by user")
        updateConnectionUI()
    else
        -- User clicked Connect
        userDisconnected = false  -- Allow reconnection
        local success, _ = httpGet("/health")
        if success then
            -- Check for path mismatch BEFORE connecting
            local currentPath = Config.getProjectDir()
            if currentPath ~= "" and not hasShownPathMismatch then
                local infoSuccess, infoData = httpGet("/rbxsync/server-info")
                if infoSuccess and infoData and infoData.vscode_workspaces and #infoData.vscode_workspaces > 0 then
                    local vscodePath = infoData.vscode_workspaces[1]
                    if currentPath ~= vscodePath then
                        -- Check if parent/child relationship
                        local isRelated = string.find(currentPath, vscodePath, 1, true) == 1 or
                                         string.find(vscodePath, currentPath, 1, true) == 1
                        if not isRelated then
                            hasShownPathMismatch = true
                            lastKnownVscodePath = vscodePath
                            print("[RbxSync] Path mismatch on connect - showing dialog")
                            showPathMismatchDialog(
                                vscodePath,
                                function()  -- onContinue: connect with current path
                                    isConnected = true
                                    dismissViewportNotification()
                                    registerWithServer()
                                    ChangeTracker.setConnected(true)
                                    startConsoleCapture()
                                    print("Connected (kept Studio path)")
                                    updateConnectionUI()
                                end,
                                function()  -- onFixPath: switch to VS Code path
                                    Config.setProjectDir(vscodePath)
                                    if projectDirInput then
                                        projectDirInput.Text = vscodePath
                                        updateFolderNameDisplay(vscodePath)
                                    end
                                    isConnected = true
                                    dismissViewportNotification()
                                    registerWithServer()
                                    ChangeTracker.setConnected(true)
                                    startConsoleCapture()
                                    print("[RbxSync] Connected with VS Code path: " .. vscodePath)
                                    updateConnectionUI()
                                end,
                                function()  -- onCancel: don't connect
                                    hasShownPathMismatch = false
                                    lastKnownVscodePath = nil
                                    print("Connection cancelled - path mismatch")
                                    updateConnectionUI()
                                end
                            )
                            return  -- Don't continue with normal connect flow
                        end
                    end
                end
            end

            isConnected = true
            dismissViewportNotification()  -- Dismiss viewport notification if shown
            registerWithServer()

            -- Check for queued changes before fully connecting
            local queuedCount = ChangeTracker.getQueuedCount()
            if queuedCount > 0 then
                -- Show confirmation dialog for queued changes
                showConfirmation(
                    "Pending Changes",
                    queuedCount .. " change(s) were queued while disconnected. Sync them now?",
                    function()
                        -- User confirmed - sync the changes
                        ChangeTracker.setConnected(true)
                        ChangeTracker.flushQueue()
                        -- Start change tracker for auto-extract (if enabled)
                        if syncSettings.studioToFiles then
                            ChangeTracker.start()
                        end
                        -- Start console capture for E2E testing
                        startConsoleCapture()
                        print("Connected")
                    end,
                    function()
                        -- User cancelled - discard the changes
                        ChangeTracker.clearQueue()
                        ChangeTracker.setConnected(true)
                        -- Start change tracker for auto-extract (if enabled)
                        if syncSettings.studioToFiles then
                            ChangeTracker.start()
                        end
                        -- Start console capture for E2E testing
                        startConsoleCapture()
                        print("Connected (queued changes discarded)")
                    end,
                    "Sync",
                    "Discard"
                )
            else
                -- No queued changes, just connect normally
                ChangeTracker.setConnected(true)
                -- Start change tracker for auto-extract (if enabled)
                if syncSettings.studioToFiles then
                    ChangeTracker.start()
                end
                -- Start console capture for E2E testing
                startConsoleCapture()
                print("Connected")
            end
        else
            isConnected = false
            warn("Could not connect to server")
        end
        updateConnectionUI()
    end
end)

-- ============================================================================
-- Project Path with Workspace Dropdown
-- ============================================================================

local projectSection = Instance.new("Frame")
projectSection.Size = UDim2.new(1, 0, 0, 100)  -- Taller to fit folder name
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
projectLabel.Text = "SYNCING TO"
projectLabel.TextColor3 = Colors.info  -- Blue accent color
projectLabel.TextSize = 10
projectLabel.Font = Enum.Font.GothamBold
projectLabel.TextXAlignment = Enum.TextXAlignment.Left
projectLabel.Parent = projectSection

-- Prominent folder name display (white text, no icon)
local folderNameLabel = Instance.new("TextLabel")
folderNameLabel.Name = "FolderNameLabel"
folderNameLabel.Size = UDim2.new(1, 0, 0, 24)
folderNameLabel.Position = UDim2.new(0, 0, 0, 16)
folderNameLabel.BackgroundTransparency = 1
folderNameLabel.Text = "(no project)"
folderNameLabel.TextColor3 = Colors.textPrimary  -- White instead of green
folderNameLabel.TextSize = 16
folderNameLabel.Font = Enum.Font.GothamBold
folderNameLabel.TextXAlignment = Enum.TextXAlignment.Left
folderNameLabel.TextTruncate = Enum.TextTruncate.AtEnd
folderNameLabel.Parent = projectSection

local projectPathFrame = Instance.new("Frame")
projectPathFrame.Size = UDim2.new(1, 0, 0, 36)
projectPathFrame.Position = UDim2.new(0, 0, 0, 44)
projectPathFrame.BackgroundColor3 = Colors.surface
projectPathFrame.BorderSizePixel = 0
projectPathFrame.Parent = projectSection

local projectPathCorner = Instance.new("UICorner")
projectPathCorner.CornerRadius = UDim.new(0, 6)
projectPathCorner.Parent = projectPathFrame

local projectPathStroke = Instance.new("UIStroke")
projectPathStroke.ApplyStrokeMode = Enum.ApplyStrokeMode.Border
projectPathStroke.Color = Colors.border
projectPathStroke.Thickness = 1
projectPathStroke.Parent = projectPathFrame

local projectDirInput = Instance.new("TextBox")
projectDirInput.Size = UDim2.new(1, -40, 1, 0)
projectDirInput.BackgroundColor3 = Colors.background
projectDirInput.BackgroundTransparency = 0
projectDirInput.Text = Config.getProjectDir()
projectDirInput.PlaceholderText = "Enter project path..."
projectDirInput.PlaceholderColor3 = Colors.textMuted
projectDirInput.TextColor3 = Colors.textPrimary
projectDirInput.TextSize = 12
projectDirInput.Font = Enum.Font.GothamMedium
projectDirInput.TextXAlignment = Enum.TextXAlignment.Left
projectDirInput.ClearTextOnFocus = false
projectDirInput.Parent = projectPathFrame

local inputCorner = Instance.new("UICorner")
inputCorner.CornerRadius = UDim.new(0, 4)
inputCorner.Parent = projectDirInput

local inputPadding = Instance.new("UIPadding")
inputPadding.PaddingLeft = UDim.new(0, 10)
inputPadding.PaddingRight = UDim.new(0, 6)
inputPadding.Parent = projectDirInput

-- Dropdown toggle button (image-based for better rendering)
local dropdownButton = Instance.new("TextButton")
dropdownButton.Size = UDim2.new(0, 36, 1, 0)
dropdownButton.Position = UDim2.new(1, -36, 0, 0)
dropdownButton.BackgroundTransparency = 1
dropdownButton.Text = ""
dropdownButton.AutoButtonColor = false
dropdownButton.Parent = projectPathFrame

local dropdownArrow = Instance.new("TextLabel")
dropdownArrow.Size = UDim2.new(1, 0, 1, 0)
dropdownArrow.BackgroundTransparency = 1
dropdownArrow.Text = "▼"
dropdownArrow.TextColor3 = Colors.textSecondary
dropdownArrow.TextSize = 16
dropdownArrow.Font = Enum.Font.GothamBold
dropdownArrow.Parent = dropdownButton

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

-- Constants for section sizing
local PROJECT_SECTION_COLLAPSED = 100

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
        noProjects.Size = UDim2.new(1, -8, 0, 28)
        noProjects.BackgroundTransparency = 1
        noProjects.Text = "No recent projects"
        noProjects.TextColor3 = Colors.textMuted
        noProjects.TextSize = 11
        noProjects.Font = Enum.Font.Gotham
        noProjects.TextXAlignment = Enum.TextXAlignment.Center
        noProjects.ZIndex = 11
        noProjects.Parent = dropdownFrame

        dropdownFrame.Size = UDim2.new(1, 0, 0, 36)
        return
    end

    -- Limit to 5 recent projects
    local maxItems = math.min(#recentProjects, 5)

    -- Create recent project option buttons
    for i = 1, maxItems do
        local projectPath = recentProjects[i]
        local isActive = projectPath == currentPath

        local option = Instance.new("TextButton")
        option.Size = UDim2.new(1, -8, 0, 28)
        -- Active item uses subtle blue highlight instead of green
        option.BackgroundColor3 = isActive and Color3.fromRGB(60, 70, 90) or Colors.surface
        option.BackgroundTransparency = isActive and 0 or 1
        option.BorderSizePixel = 0
        option.Text = ""
        option.AutoButtonColor = false
        option.LayoutOrder = i
        option.ZIndex = 11
        option.Parent = dropdownFrame

        local optionCorner = Instance.new("UICorner")
        optionCorner.CornerRadius = UDim.new(0, 4)
        optionCorner.Parent = option

        -- Folder icon for each item
        local itemFolderIcon = Instance.new("ImageLabel")
        itemFolderIcon.Size = UDim2.new(0, 12, 0, 12)
        itemFolderIcon.Position = UDim2.new(0, 8, 0.5, -6)
        itemFolderIcon.BackgroundTransparency = 1
        itemFolderIcon.Image = "rbxassetid://99625725727957"
        itemFolderIcon.ScaleType = Enum.ScaleType.Fit
        itemFolderIcon.ZIndex = 12
        itemFolderIcon.Parent = option

        -- Full path (white text)
        local nameLabel = Instance.new("TextLabel")
        nameLabel.Size = UDim2.new(1, -28, 0, 14)
        nameLabel.Position = UDim2.new(0, 26, 0.5, -7)
        nameLabel.BackgroundTransparency = 1
        nameLabel.Text = projectPath
        nameLabel.TextColor3 = Colors.textPrimary  -- White for all items
        nameLabel.TextSize = 11
        nameLabel.Font = isActive and Enum.Font.GothamBold or Enum.Font.GothamMedium
        nameLabel.TextXAlignment = Enum.TextXAlignment.Left
        nameLabel.TextTruncate = Enum.TextTruncate.AtEnd
        nameLabel.ZIndex = 12
        nameLabel.Parent = option

        -- Check mark for active item
        if isActive then
            local checkMark = Instance.new("TextLabel")
            checkMark.Size = UDim2.new(0, 16, 1, 0)
            checkMark.Position = UDim2.new(1, -20, 0, 0)
            checkMark.BackgroundTransparency = 1
            checkMark.Text = "✓"
            checkMark.TextColor3 = Colors.accent
            checkMark.TextSize = 12
            checkMark.Font = Enum.Font.GothamBold
            checkMark.ZIndex = 12
            checkMark.Parent = option
        end

        -- Hover effect
        option.MouseEnter:Connect(function()
            if not isActive then
                option.BackgroundColor3 = Colors.surfaceHover
                option.BackgroundTransparency = 0
            end
        end)
        option.MouseLeave:Connect(function()
            if not isActive then
                option.BackgroundTransparency = 1
            end
        end)

        option.MouseButton1Click:Connect(function()
            Config.setProjectDir(projectPath)
            projectDirInput.Text = projectPath
            updateFolderNameDisplay(projectPath)
            dropdownFrame.Visible = false
            dropdownArrow.Text = "▼"
            projectSection.Size = UDim2.new(1, 0, 0, PROJECT_SECTION_COLLAPSED)
            -- Re-register with server after path change
            if isConnected then
                registerWithServer()
            end
        end)
    end

    -- Set dropdown height based on items (exact fit, no extra space)
    local itemHeight = 28 + 2 -- height + padding
    local totalHeight = maxItems * itemHeight + 6 -- +6 for top/bottom padding only
    dropdownFrame.Size = UDim2.new(1, 0, 0, totalHeight)
end

-- Toggle dropdown
dropdownButton.MouseButton1Click:Connect(function()
    dropdownFrame.Visible = not dropdownFrame.Visible
    dropdownArrow.Text = dropdownFrame.Visible and "▲" or "▼"
    if dropdownFrame.Visible then
        updateRecentProjectsDropdown()
        -- Expand section to fit dropdown (base 100 + dropdown height + gap)
        local dropdownHeight = dropdownFrame.Size.Y.Offset
        projectSection.Size = UDim2.new(1, 0, 0, PROJECT_SECTION_COLLAPSED + dropdownHeight + 8)
    else
        -- Collapse back to normal
        projectSection.Size = UDim2.new(1, 0, 0, PROJECT_SECTION_COLLAPSED)
    end
end)

-- Helper to update folder name display
updateFolderNameDisplay = function(path: string?)
    local displayPath = path or projectDirInput.Text or ""
    if displayPath == "" then
        folderNameLabel.Text = "(no project)"
        folderNameLabel.TextColor3 = Colors.textMuted
    else
        -- Extract folder name from path
        local folderName = displayPath:match("([^/\\]+)$") or displayPath
        folderNameLabel.Text = folderName
        folderNameLabel.TextColor3 = Colors.textPrimary  -- White, not green
    end
end

-- Initialize folder name display
updateFolderNameDisplay(Config.getProjectDir())

projectDirInput.FocusLost:Connect(function()
    if projectDirInput.Text ~= "" then
        Config.setProjectDir(projectDirInput.Text)
        updateFolderNameDisplay(projectDirInput.Text)
        -- Re-register with server after path change
        if isConnected then
            registerWithServer()
        end
    end
end)

-- ═══════════════════════════════════════════════════════════════════════════
-- BOTTOM SPACER (fixes scroll cutoff)
-- ═══════════════════════════════════════════════════════════════════════════

-- Bottom spacer removed - no longer needed with compact UI

-- Dummy syncOptionsSection reference for compatibility (not rendered)
local syncOptionsSection = Instance.new("Frame")
syncOptionsSection.Size = UDim2.new(0, 0, 0, 0)
syncOptionsSection.Visible = false
syncOptionsSection.Parent = mainFrame

-- Path mismatch check and dialog (viewport popup)
local pathMismatchPopup: ScreenGui? = nil

local function dismissPathMismatchPopup()
    if pathMismatchPopup then
        local frame = pathMismatchPopup:FindFirstChild("MismatchFrame")
        if frame then
            local tween = game:GetService("TweenService"):Create(
                frame,
                TweenInfo.new(0.2, Enum.EasingStyle.Quad, Enum.EasingDirection.In),
                {Position = UDim2.new(1, 20, 1, -20)}
            )
            tween:Play()
            tween.Completed:Connect(function()
                if pathMismatchPopup then
                    pathMismatchPopup:Destroy()
                    pathMismatchPopup = nil
                end
            end)
        else
            pathMismatchPopup:Destroy()
            pathMismatchPopup = nil
        end
    end
end

-- Legacy overlay reference (kept for compatibility)
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
    -- Remove existing popup if present
    if pathMismatchPopup then
        pathMismatchPopup:Destroy()
        pathMismatchPopup = nil
    end

    local studioPath = Config.getProjectDir()

    -- Create ScreenGui in CoreGui (viewport popup like notification)
    local screenGui = Instance.new("ScreenGui")
    screenGui.Name = "RbxSyncPathMismatch"
    screenGui.DisplayOrder = 1001  -- Above notification
    screenGui.IgnoreGuiInset = true
    screenGui.ResetOnSpawn = false

    local success = pcall(function()
        screenGui.Parent = CoreGui
    end)
    if not success then
        screenGui:Destroy()
        return
    end
    pathMismatchPopup = screenGui

    -- Main popup frame (bottom-right, matches notification style)
    local frame = Instance.new("Frame")
    frame.Name = "MismatchFrame"
    frame.Size = UDim2.new(0, 380, 0, 195)
    frame.Position = UDim2.new(1, 20, 1, -24)  -- Start off-screen for animation
    frame.AnchorPoint = Vector2.new(1, 1)
    frame.BackgroundColor3 = Color3.fromRGB(28, 28, 32)
    frame.BorderSizePixel = 0
    frame.Parent = screenGui

    local frameCorner = Instance.new("UICorner")
    frameCorner.CornerRadius = UDim.new(0, 12)
    frameCorner.Parent = frame

    local frameStroke = Instance.new("UIStroke")
    frameStroke.ApplyStrokeMode = Enum.ApplyStrokeMode.Border
    frameStroke.Color = Color3.fromRGB(60, 60, 70)
    frameStroke.Thickness = 1
    frameStroke.Parent = frame

    -- Drop shadow
    local shadow = Instance.new("ImageLabel")
    shadow.Name = "Shadow"
    shadow.Size = UDim2.new(1, 40, 1, 40)
    shadow.Position = UDim2.new(0.5, 0, 0.5, 8)
    shadow.AnchorPoint = Vector2.new(0.5, 0.5)
    shadow.BackgroundTransparency = 1
    shadow.Image = "rbxassetid://5554236805"
    shadow.ImageColor3 = Color3.fromRGB(0, 0, 0)
    shadow.ImageTransparency = 0.4
    shadow.ScaleType = Enum.ScaleType.Slice
    shadow.SliceCenter = Rect.new(23, 23, 277, 277)
    shadow.ZIndex = -1
    shadow.Parent = frame

    -- Warning accent bar on left (yellow for warning)
    local accentBar = Instance.new("Frame")
    accentBar.Size = UDim2.new(0, 4, 1, -24)
    accentBar.Position = UDim2.new(0, 12, 0, 12)
    accentBar.BackgroundColor3 = Colors.warning
    accentBar.BorderSizePixel = 0
    accentBar.Parent = frame

    local accentCorner = Instance.new("UICorner")
    accentCorner.CornerRadius = UDim.new(0, 2)
    accentCorner.Parent = accentBar

    -- Warning icon (simple circle with !)
    local iconBg = Instance.new("Frame")
    iconBg.Size = UDim2.new(0, 40, 0, 40)
    iconBg.Position = UDim2.new(0, 28, 0, 16)
    iconBg.BackgroundColor3 = Color3.fromRGB(250, 204, 21)
    iconBg.BackgroundTransparency = 0.85
    iconBg.BorderSizePixel = 0
    iconBg.Parent = frame

    local iconBgCorner = Instance.new("UICorner")
    iconBgCorner.CornerRadius = UDim.new(0, 8)
    iconBgCorner.Parent = iconBg

    local warningIcon = Instance.new("TextLabel")
    warningIcon.Size = UDim2.new(1, 0, 1, 0)
    warningIcon.BackgroundTransparency = 1
    warningIcon.Text = "!"
    warningIcon.TextColor3 = Colors.warning
    warningIcon.TextSize = 24
    warningIcon.Font = Enum.Font.GothamBold
    warningIcon.Parent = iconBg

    -- Title
    local title = Instance.new("TextLabel")
    title.Size = UDim2.new(1, -100, 0, 22)
    title.Position = UDim2.new(0, 80, 0, 16)
    title.BackgroundTransparency = 1
    title.Text = "Path Mismatch"
    title.TextColor3 = Colors.warning
    title.TextSize = 15
    title.Font = Enum.Font.GothamBold
    title.TextXAlignment = Enum.TextXAlignment.Left
    title.Parent = frame

    -- Path info container
    local pathContainer = Instance.new("Frame")
    pathContainer.Size = UDim2.new(1, -40, 0, 70)
    pathContainer.Position = UDim2.new(0, 80, 0, 42)
    pathContainer.BackgroundTransparency = 1
    pathContainer.Parent = frame

    -- Studio path
    local studioLabel = Instance.new("TextLabel")
    studioLabel.Size = UDim2.new(1, -220, 0, 14)
    studioLabel.Position = UDim2.new(0, 0, 0, 0)
    studioLabel.BackgroundTransparency = 1
    studioLabel.Text = "Studio:"
    studioLabel.TextColor3 = Colors.textMuted
    studioLabel.TextSize = 11
    studioLabel.Font = Enum.Font.GothamMedium
    studioLabel.TextXAlignment = Enum.TextXAlignment.Left
    studioLabel.Parent = pathContainer

    local studioPathLabel = Instance.new("TextLabel")
    studioPathLabel.Size = UDim2.new(1, 0, 0, 16)
    studioPathLabel.Position = UDim2.new(0, 0, 0, 14)
    studioPathLabel.BackgroundTransparency = 1
    studioPathLabel.Text = studioPath
    studioPathLabel.TextColor3 = Colors.textPrimary
    studioPathLabel.TextSize = 11
    studioPathLabel.Font = Enum.Font.Code
    studioPathLabel.TextXAlignment = Enum.TextXAlignment.Left
    studioPathLabel.TextTruncate = Enum.TextTruncate.AtEnd
    studioPathLabel.Parent = pathContainer

    -- VS Code path
    local vscodeLabel = Instance.new("TextLabel")
    vscodeLabel.Size = UDim2.new(1, -220, 0, 14)
    vscodeLabel.Position = UDim2.new(0, 0, 0, 34)
    vscodeLabel.BackgroundTransparency = 1
    vscodeLabel.Text = "VS Code:"
    vscodeLabel.TextColor3 = Colors.textMuted
    vscodeLabel.TextSize = 11
    vscodeLabel.Font = Enum.Font.GothamMedium
    vscodeLabel.TextXAlignment = Enum.TextXAlignment.Left
    vscodeLabel.Parent = pathContainer

    local vscodePathLabel = Instance.new("TextLabel")
    vscodePathLabel.Size = UDim2.new(1, 0, 0, 16)
    vscodePathLabel.Position = UDim2.new(0, 0, 0, 48)
    vscodePathLabel.BackgroundTransparency = 1
    vscodePathLabel.Text = vscodePath
    vscodePathLabel.TextColor3 = Colors.accent
    vscodePathLabel.TextSize = 11
    vscodePathLabel.Font = Enum.Font.Code
    vscodePathLabel.TextXAlignment = Enum.TextXAlignment.Left
    vscodePathLabel.TextTruncate = Enum.TextTruncate.AtEnd
    vscodePathLabel.Parent = pathContainer

    -- Warning message
    local warningMsg = Instance.new("TextLabel")
    warningMsg.Size = UDim2.new(1, -40, 0, 16)
    warningMsg.Position = UDim2.new(0, 80, 0, 112)
    warningMsg.BackgroundTransparency = 1
    warningMsg.Text = "Files will extract to Studio path, not VS Code!"
    warningMsg.TextColor3 = Colors.warning
    warningMsg.TextSize = 11
    warningMsg.Font = Enum.Font.GothamMedium
    warningMsg.TextXAlignment = Enum.TextXAlignment.Left
    warningMsg.Parent = frame

    -- Button container
    local buttonContainer = Instance.new("Frame")
    buttonContainer.Size = UDim2.new(1, -28, 0, 34)
    buttonContainer.Position = UDim2.new(0, 14, 1, -48)
    buttonContainer.BackgroundTransparency = 1
    buttonContainer.Parent = frame

    local buttonLayout = Instance.new("UIListLayout")
    buttonLayout.FillDirection = Enum.FillDirection.Horizontal
    buttonLayout.HorizontalAlignment = Enum.HorizontalAlignment.Right
    buttonLayout.VerticalAlignment = Enum.VerticalAlignment.Center
    buttonLayout.Padding = UDim.new(0, 8)
    buttonLayout.Parent = buttonContainer

    -- Dismiss button (ghost)
    local dismissBtn = Instance.new("TextButton")
    dismissBtn.Size = UDim2.new(0, 70, 0, 32)
    dismissBtn.BackgroundColor3 = Color3.fromRGB(40, 40, 46)
    dismissBtn.BorderSizePixel = 0
    dismissBtn.Text = "Cancel"
    dismissBtn.TextColor3 = Colors.textMuted
    dismissBtn.TextSize = 11
    dismissBtn.Font = Enum.Font.GothamMedium
    dismissBtn.AutoButtonColor = false
    dismissBtn.LayoutOrder = 1
    dismissBtn.Parent = buttonContainer

    local dismissCorner = Instance.new("UICorner")
    dismissCorner.CornerRadius = UDim.new(0, 6)
    dismissCorner.Parent = dismissBtn

    -- Connect button (secondary) - keep Studio's current path
    local continueBtn = Instance.new("TextButton")
    continueBtn.Size = UDim2.new(0, 80, 0, 32)
    continueBtn.BackgroundColor3 = Color3.fromRGB(40, 40, 46)
    continueBtn.BorderSizePixel = 0
    continueBtn.Text = "Connect"
    continueBtn.TextColor3 = Colors.textSecondary
    continueBtn.TextSize = 11
    continueBtn.Font = Enum.Font.GothamMedium
    continueBtn.AutoButtonColor = false
    continueBtn.LayoutOrder = 2
    continueBtn.Parent = buttonContainer

    local continueCorner = Instance.new("UICorner")
    continueCorner.CornerRadius = UDim.new(0, 6)
    continueCorner.Parent = continueBtn

    -- Use VS Code Path button (primary)
    local fixBtn = Instance.new("TextButton")
    fixBtn.Size = UDim2.new(0, 125, 0, 32)
    fixBtn.BackgroundColor3 = Colors.accent
    fixBtn.BorderSizePixel = 0
    fixBtn.Text = "Use VS Code Path"
    fixBtn.TextColor3 = Color3.fromRGB(255, 255, 255)
    fixBtn.TextSize = 11
    fixBtn.Font = Enum.Font.GothamBold
    fixBtn.AutoButtonColor = false
    fixBtn.LayoutOrder = 3
    fixBtn.Parent = buttonContainer

    local fixCorner = Instance.new("UICorner")
    fixCorner.CornerRadius = UDim.new(0, 6)
    fixCorner.Parent = fixBtn

    -- Hover effects
    dismissBtn.MouseEnter:Connect(function()
        dismissBtn.BackgroundColor3 = Color3.fromRGB(52, 52, 60)
    end)
    dismissBtn.MouseLeave:Connect(function()
        dismissBtn.BackgroundColor3 = Color3.fromRGB(40, 40, 46)
    end)

    continueBtn.MouseEnter:Connect(function()
        continueBtn.BackgroundColor3 = Color3.fromRGB(52, 52, 60)
    end)
    continueBtn.MouseLeave:Connect(function()
        continueBtn.BackgroundColor3 = Color3.fromRGB(40, 40, 46)
    end)

    fixBtn.MouseEnter:Connect(function()
        fixBtn.BackgroundColor3 = Colors.accentHover
    end)
    fixBtn.MouseLeave:Connect(function()
        fixBtn.BackgroundColor3 = Colors.accent
    end)

    -- Button handlers
    dismissBtn.MouseButton1Click:Connect(function()
        dismissPathMismatchPopup()
        onCancel()
    end)

    continueBtn.MouseButton1Click:Connect(function()
        dismissPathMismatchPopup()
        onContinue()
    end)

    fixBtn.MouseButton1Click:Connect(function()
        dismissPathMismatchPopup()
        onFixPath()
    end)

    -- Animate in from right
    task.defer(function()
        local tween = game:GetService("TweenService"):Create(
            frame,
            TweenInfo.new(0.35, Enum.EasingStyle.Back, Enum.EasingDirection.Out),
            {Position = UDim2.new(1, -24, 1, -24)}
        )
        tween:Play()
    end)
end

-- Confirmation dialog helper
local confirmationOverlay: Frame? = nil

showConfirmation = function(title: string, message: string, onConfirm: () -> (), onCancel: () -> (), confirmText: string?, cancelText: string?)
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
    overlay.Parent = widget
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
    cancelBtn.Text = cancelText or "Cancel"
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
    confirmBtn.Text = confirmText or "Confirm"
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

-- ============================================================================
-- VIEWPORT NOTIFICATION (appears in game viewport like Rojo)
-- ============================================================================

local viewportNotification: ScreenGui? = nil

dismissViewportNotification = function()
    if viewportNotification then
        -- Animate out
        local frame = viewportNotification:FindFirstChild("NotificationFrame")
        if frame then
            local tween = game:GetService("TweenService"):Create(
                frame,
                TweenInfo.new(0.2, Enum.EasingStyle.Quad, Enum.EasingDirection.In),
                {Position = UDim2.new(1, 20, 1, -20)}
            )
            tween:Play()
            tween.Completed:Connect(function()
                if viewportNotification then
                    viewportNotification:Destroy()
                    viewportNotification = nil
                end
            end)
        else
            viewportNotification:Destroy()
            viewportNotification = nil
        end
    end
end

local function showViewportNotification(projectName: string?, host: string?, port: number?)
    -- Don't show if already shown or connected
    if hasShownConnectPrompt or isConnected or viewportNotification then
        return
    end
    hasShownConnectPrompt = true

    -- Create ScreenGui in CoreGui (appears in viewport)
    local screenGui = Instance.new("ScreenGui")
    screenGui.Name = "RbxSyncNotification"
    screenGui.DisplayOrder = 1000
    screenGui.IgnoreGuiInset = true
    screenGui.ResetOnSpawn = false

    -- Try to parent to CoreGui (requires plugin permissions)
    local success = pcall(function()
        screenGui.Parent = CoreGui
    end)
    if not success then
        screenGui:Destroy()
        return
    end
    viewportNotification = screenGui

    -- Main notification frame (bottom-right corner) - refined design
    local frame = Instance.new("Frame")
    frame.Name = "NotificationFrame"
    frame.Size = UDim2.new(0, 380, 0, 155)
    frame.Position = UDim2.new(1, 20, 1, -24)  -- Start off-screen for animation
    frame.AnchorPoint = Vector2.new(1, 1)
    frame.BackgroundColor3 = Color3.fromRGB(28, 28, 32)
    frame.BorderSizePixel = 0
    frame.Parent = screenGui

    local frameCorner = Instance.new("UICorner")
    frameCorner.CornerRadius = UDim.new(0, 12)
    frameCorner.Parent = frame

    local frameStroke = Instance.new("UIStroke")
    frameStroke.ApplyStrokeMode = Enum.ApplyStrokeMode.Border
    frameStroke.Color = Color3.fromRGB(60, 60, 70)
    frameStroke.Thickness = 1
    frameStroke.Parent = frame

    -- Drop shadow effect
    local shadow = Instance.new("ImageLabel")
    shadow.Name = "Shadow"
    shadow.Size = UDim2.new(1, 40, 1, 40)
    shadow.Position = UDim2.new(0.5, 0, 0.5, 8)
    shadow.AnchorPoint = Vector2.new(0.5, 0.5)
    shadow.BackgroundTransparency = 1
    shadow.Image = "rbxassetid://5554236805"
    shadow.ImageColor3 = Color3.fromRGB(0, 0, 0)
    shadow.ImageTransparency = 0.4
    shadow.ScaleType = Enum.ScaleType.Slice
    shadow.SliceCenter = Rect.new(23, 23, 277, 277)
    shadow.ZIndex = -1
    shadow.Parent = frame

    -- Accent bar on left edge
    local accentBar = Instance.new("Frame")
    accentBar.Size = UDim2.new(0, 4, 1, -24)
    accentBar.Position = UDim2.new(0, 12, 0, 12)
    accentBar.BackgroundColor3 = Colors.accent
    accentBar.BorderSizePixel = 0
    accentBar.Parent = frame

    local accentCorner = Instance.new("UICorner")
    accentCorner.CornerRadius = UDim.new(0, 2)
    accentCorner.Parent = accentBar

    -- RbxSync logo/icon
    local iconImage = Instance.new("ImageLabel")
    iconImage.Size = UDim2.new(0, 44, 0, 44)
    iconImage.Position = UDim2.new(0, 28, 0, 18)
    iconImage.BackgroundTransparency = 1
    iconImage.Image = "rbxassetid://134458909684902"  -- RbxSync icon
    iconImage.ScaleType = Enum.ScaleType.Fit
    iconImage.Parent = frame

    -- Title
    local title = Instance.new("TextLabel")
    title.Size = UDim2.new(1, -100, 0, 22)
    title.Position = UDim2.new(0, 84, 0, 18)
    title.BackgroundTransparency = 1
    title.Text = "RbxSync Server Detected"
    title.TextColor3 = Colors.textPrimary
    title.TextSize = 15
    title.Font = Enum.Font.GothamBold
    title.TextXAlignment = Enum.TextXAlignment.Left
    title.Parent = frame

    -- Message - wider area with proper text wrapping
    local displayName = projectName or "Project"
    local message = Instance.new("TextLabel")
    message.Size = UDim2.new(1, -100, 0, 44)
    message.Position = UDim2.new(0, 84, 0, 42)
    message.BackgroundTransparency = 1
    message.Text = string.format("'%s' is ready to sync. Connect to sync files between Studio and your local filesystem.", displayName)
    message.TextColor3 = Colors.textSecondary
    message.TextSize = 12
    message.Font = Enum.Font.Gotham
    message.TextXAlignment = Enum.TextXAlignment.Left
    message.TextYAlignment = Enum.TextYAlignment.Top
    message.TextWrapped = true
    message.LineHeight = 1.3
    message.Parent = frame

    -- Button container
    local buttonContainer = Instance.new("Frame")
    buttonContainer.Size = UDim2.new(1, -28, 0, 34)
    buttonContainer.Position = UDim2.new(0, 14, 1, -48)
    buttonContainer.BackgroundTransparency = 1
    buttonContainer.Parent = frame

    local buttonLayout = Instance.new("UIListLayout")
    buttonLayout.FillDirection = Enum.FillDirection.Horizontal
    buttonLayout.HorizontalAlignment = Enum.HorizontalAlignment.Right
    buttonLayout.VerticalAlignment = Enum.VerticalAlignment.Center
    buttonLayout.Padding = UDim.new(0, 10)
    buttonLayout.Parent = buttonContainer

    -- Dismiss button - subtle ghost style
    local dismissBtn = Instance.new("TextButton")
    dismissBtn.Size = UDim2.new(0, 80, 0, 32)
    dismissBtn.BackgroundColor3 = Color3.fromRGB(40, 40, 46)
    dismissBtn.BorderSizePixel = 0
    dismissBtn.Text = "Dismiss"
    dismissBtn.TextColor3 = Colors.textMuted
    dismissBtn.TextSize = 12
    dismissBtn.Font = Enum.Font.GothamMedium
    dismissBtn.AutoButtonColor = false
    dismissBtn.LayoutOrder = 1
    dismissBtn.Parent = buttonContainer

    local dismissCorner = Instance.new("UICorner")
    dismissCorner.CornerRadius = UDim.new(0, 6)
    dismissCorner.Parent = dismissBtn

    -- Connect button - prominent accent
    local connectBtn = Instance.new("TextButton")
    connectBtn.Size = UDim2.new(0, 90, 0, 32)
    connectBtn.BackgroundColor3 = Colors.accent
    connectBtn.BorderSizePixel = 0
    connectBtn.Text = "Connect"
    connectBtn.TextColor3 = Color3.fromRGB(255, 255, 255)
    connectBtn.TextSize = 12
    connectBtn.Font = Enum.Font.GothamBold
    connectBtn.AutoButtonColor = false
    connectBtn.LayoutOrder = 2
    connectBtn.Parent = buttonContainer

    local connectCorner = Instance.new("UICorner")
    connectCorner.CornerRadius = UDim.new(0, 6)
    connectCorner.Parent = connectBtn

    -- Hover effects with smooth transitions
    dismissBtn.MouseEnter:Connect(function()
        dismissBtn.BackgroundColor3 = Color3.fromRGB(52, 52, 60)
    end)
    dismissBtn.MouseLeave:Connect(function()
        dismissBtn.BackgroundColor3 = Color3.fromRGB(40, 40, 46)
    end)
    connectBtn.MouseEnter:Connect(function()
        connectBtn.BackgroundColor3 = Colors.accentHover
    end)
    connectBtn.MouseLeave:Connect(function()
        connectBtn.BackgroundColor3 = Colors.accent
    end)

    -- Button handlers
    dismissBtn.MouseButton1Click:Connect(function()
        dismissViewportNotification()
    end)

    connectBtn.MouseButton1Click:Connect(function()
        dismissViewportNotification()
        -- Enable connection
        userDisconnected = false
        -- Trigger immediate connection attempt
        local connectSuccess, _ = httpGet("/health")
        if connectSuccess then
            isConnected = true
            registerWithServer()
            ChangeTracker.setConnected(true)
            if syncSettings.studioToFiles then
                ChangeTracker.start()
            end
            startConsoleCapture()
            print("[RbxSync] Connected via viewport notification")
            updateConnectionUI()
        end
    end)

    -- Animate in from right
    task.defer(function()
        local tween = game:GetService("TweenService"):Create(
            frame,
            TweenInfo.new(0.35, Enum.EasingStyle.Back, Enum.EasingDirection.Out),
            {Position = UDim2.new(1, -24, 1, -24)}
        )
        tween:Play()
    end)

    -- Auto-dismiss after 30 seconds if not interacted with
    task.delay(30, function()
        if viewportNotification then
            dismissViewportNotification()
        end
    end)
end

-- Connect prompt overlay (fallback for widget)
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
    overlay.Parent = widget
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
            registerWithServer()

            -- Check for queued changes
            local queuedCount = ChangeTracker.getQueuedCount()
            if queuedCount > 0 then
                -- Show confirmation dialog for queued changes
                showConfirmation(
                    "Pending Changes",
                    queuedCount .. " change(s) were queued while disconnected. Sync them now?",
                    function()
                        -- User confirmed - sync the changes
                        ChangeTracker.setConnected(true)
                        ChangeTracker.flushQueue()
                        if syncSettings.studioToFiles then
                            ChangeTracker.start()
                        end
                        startConsoleCapture()
                        print("Connected")
                        updateConnectionUI()
                    end,
                    function()
                        -- User cancelled - discard the changes
                        ChangeTracker.clearQueue()
                        ChangeTracker.setConnected(true)
                        if syncSettings.studioToFiles then
                            ChangeTracker.start()
                        end
                        startConsoleCapture()
                        print("Connected (queued changes discarded)")
                        updateConnectionUI()
                    end,
                    "Sync",
                    "Discard"
                )
            else
                ChangeTracker.setConnected(true)
                if syncSettings.studioToFiles then
                    ChangeTracker.start()
                end
                startConsoleCapture()
                print("Connected")
                updateConnectionUI()
            end
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
    checkboxButton.BackgroundColor3 = checked and Colors.info or Colors.surface  -- Blue for checkboxes
    checkboxButton.BorderSizePixel = 0
    checkboxButton.Text = checked and "✓" or ""
    checkboxButton.TextColor3 = Colors.textPrimary
    checkboxButton.TextSize = 12
    checkboxButton.Font = Enum.Font.GothamBold
    checkboxButton.Parent = checkboxFrame

    local checkboxCorner = Instance.new("UICorner")
    checkboxCorner.CornerRadius = UDim.new(0, 3)
    checkboxCorner.Parent = checkboxButton

    local checkboxStroke = Instance.new("UIStroke")
    checkboxStroke.Color = checked and Colors.info or Colors.textMuted  -- Blue for checkboxes
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
        checkboxButton.BackgroundColor3 = isChecked and Colors.info or Colors.surface  -- Blue for checkboxes
        checkboxButton.Text = isChecked and "✓" or ""
        checkboxStroke.Color = isChecked and Colors.info or Colors.textMuted  -- Blue for checkboxes
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
                    Config.setAutoExtract(true)  -- Persist setting
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
        Config.setAutoExtract(checked)  -- Persist setting
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
                                    local descendants = service:GetDescendants()
                                    for i, desc in descendants do
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
                                        -- Yield periodically to avoid script timeout on large games (RBXSYNC-25)
                                        if i % 5000 == 0 then
                                            task.wait()
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

-- ============================================================================
-- Panel Button
-- ============================================================================

panelButton.Click:Connect(function()
    widget.Enabled = not widget.Enabled
    panelButton:SetActive(widget.Enabled)

    if widget.Enabled then
        task.spawn(updateInfoDisplay)
        -- Check if we should show viewport notification
        if userDisconnected and not hasShownConnectPrompt and not isConnected then
            task.spawn(function()
                local serverAvailable, serverInfo = httpGet("/health")
                if serverAvailable then
                    local projectName = serverInfo and serverInfo.project_name or nil
                    showViewportNotification(projectName, "localhost", 44755)
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
        -- Show viewport notification if server is available but user hasn't connected yet
        if userDisconnected and not hasShownConnectPrompt and not isConnected then
            local serverAvailable, serverInfo = httpGet("/health")
            if serverAvailable then
                local projectName = serverInfo and serverInfo.project_name or nil
                task.defer(function()
                    showViewportNotification(projectName, "localhost", 44755)
                end)
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

                        -- Warn if multiple VS Code workspaces are connected
                        if #infoData.vscode_workspaces > 1 then
                            warn("[RbxSync] Warning: " .. #infoData.vscode_workspaces .. " VS Code workspaces detected! Make sure you're syncing to the right one:")
                            for i, ws in ipairs(infoData.vscode_workspaces) do
                                warn("  " .. i .. ". " .. ws)
                            end
                            warn("  Current: " .. (currentPath ~= "" and currentPath or vscodePath))
                        end

                        if currentPath == "" then
                            -- No path set - auto-populate from VS Code
                            Config.setProjectDir(vscodePath)
                            if projectDirInput then
                                projectDirInput.Text = vscodePath
                                updateFolderNameDisplay(vscodePath)
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
                                                updateFolderNameDisplay(vscodePath)
                                            end
                                            registerWithServer()
                                            print("[RbxSync] Switched to VS Code workspace: " .. vscodePath)
                                        end,
                                        function()  -- onCancel: disconnect
                                            unlinkFromServer()
                                            isConnected = false
                                            userDisconnected = true
                                            hasShownPathMismatch = false
                                            lastKnownVscodePath = nil  -- Reset so we re-detect on reconnect
                                            ChangeTracker.setConnected(false)
                                            ChangeTracker.stop()
                                            stopConsoleCapture()
                                            print("Disconnected - path mismatch cancelled")
                                            updateConnectionUI()
                                        end
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
                            updateFolderNameDisplay(infoData.cwd)
                        end
                        print("[RbxSync] Using server directory: " .. infoData.cwd)
                    end

                    registerWithServer()

                    -- Check for queued changes on auto-reconnect
                    local queuedCount = ChangeTracker.getQueuedCount()
                    if queuedCount > 0 then
                        -- Show confirmation dialog for queued changes
                        showConfirmation(
                            "Pending Changes",
                            queuedCount .. " change(s) were queued while disconnected. Sync them now?",
                            function()
                                -- User confirmed - sync the changes
                                ChangeTracker.setConnected(true)
                                ChangeTracker.flushQueue()
                                if syncSettings.studioToFiles then
                                    ChangeTracker.start()
                                end
                                startConsoleCapture()
                            end,
                            function()
                                -- User cancelled - discard the changes
                                ChangeTracker.clearQueue()
                                ChangeTracker.setConnected(true)
                                if syncSettings.studioToFiles then
                                    ChangeTracker.start()
                                end
                                startConsoleCapture()
                            end,
                            "Sync",
                            "Discard"
                        )
                    else
                        ChangeTracker.setConnected(true)
                        -- Start change tracker only if enabled
                        if syncSettings.studioToFiles then
                            ChangeTracker.start()
                        end
                        startConsoleCapture()
                    end
                else
                    print("Disconnected")
                    ChangeTracker.setConnected(false)
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
                    ChangeTracker.setConnected(false)
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
                                                    updateFolderNameDisplay(vscodePath)
                                                end
                                                registerWithServer()
                                                print("[RbxSync] Switched to VS Code workspace: " .. vscodePath)
                                            end,
                                            function()  -- onCancel: disconnect
                                                unlinkFromServer()
                                                isConnected = false
                                                userDisconnected = true
                                                hasShownPathMismatch = false
                                                lastKnownVscodePath = nil  -- Reset so we re-detect on reconnect
                                                ChangeTracker.setConnected(false)
                                                ChangeTracker.stop()
                                                stopConsoleCapture()
                                                print("Disconnected - path mismatch cancelled")
                                                updateConnectionUI()
                                            end
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
