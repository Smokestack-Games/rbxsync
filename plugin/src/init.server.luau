--!strict
--[[
    RbxSync Studio Plugin

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
local CHUNK_SIZE = 50 -- instances per chunk (Roblox HttpService has 1MB limit)

-- Load modules (modules are children of this script in the bundled .rbxm)
local Reflection = require(script.Reflection)
local Serializer = require(script.Serializer)
local Sync = require(script.Sync)
local Config = require(script.Config)
local TestRunner = require(script.TestRunner)
local ChangeTracker = require(script.ChangeTracker)

-- Initialize config with plugin reference
Config.init(plugin)

-- Plugin state
local isConnected = false
local isExtracting = false
local isSyncing = false
local userDisconnected = false  -- True when user explicitly clicks Disconnect (prevents auto-reconnect)
local cachedPlaceName: string? = nil
local lastExtractTime: number? = nil  -- Track last extraction time for estimates
local lastInstanceCount: number? = nil  -- Track last instance count
local hasLoggedLink: boolean = false  -- Prevent spam logging of "Linked to" messages

-- Console capture state (for E2E testing mode)
local consoleCapture = {
    enabled = false,
    connection = nil :: RBXScriptConnection?,
    flushThread = nil :: thread?,
    pendingMessages = {} :: {{timestamp: string, message_type: string, message: string, source: string?}},
    lastFlush = 0,
    FLUSH_INTERVAL = 0.5,  -- Flush every 500ms
}

-- Sync direction settings (disabled by default for safety)
local syncSettings = {
    studioToFiles = false,  -- Auto-extract Studio changes to files
    filesToStudio = false,  -- Sync file changes to Studio
}

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

-- Extract full game
local function extractGame(config: {any})
    if isExtracting then
        return
    end

    local projectDir = Config.getProjectDir()
    if projectDir == "" then
        warn("No project set. Enter path above.")
        return
    end

    isExtracting = true
    setStatus("Extracting...", Color3.fromRGB(100, 180, 255), 0)  -- 0 = don't auto-clear
    local startTime = os.clock()

    -- Initialize reflection engine with API dump
    local apiDump = Reflection.loadAPIDump()
    if not apiDump then
        warn("Can't load API. Try reopening Studio.")
        isExtracting = false
        return
    end

    -- Determine which services to extract
    local servicesToExtract = config.services or {
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
    }

    -- Collect all instances to serialize
    local allInstances: {Instance} = {}

    for _, serviceName in servicesToExtract do
        local service = game:FindFirstChild(serviceName)
        if service then
            table.insert(allInstances, service)
            for _, descendant in service:GetDescendants() do
                table.insert(allInstances, descendant)
            end
        end
    end

    -- Calculate chunks
    local totalChunks = math.ceil(#allInstances / CHUNK_SIZE)
    local sessionId = HttpService:GenerateGUID(false)

    -- Serialize and send in chunks (silently)
    for chunkIndex = 0, totalChunks - 1 do
        local startIdx = chunkIndex * CHUNK_SIZE + 1
        local endIdx = math.min(startIdx + CHUNK_SIZE - 1, #allInstances)

        local chunkInstances = {}
        for i = startIdx, endIdx do
            local instance = allInstances[i]
            local serialized = Serializer.serializeInstance(instance, apiDump)
            if serialized then
                table.insert(chunkInstances, serialized)
            end
        end

        local success = sendChunk(sessionId, chunkIndex, totalChunks, chunkInstances, projectDir)
        if not success then
            warn("Extraction failed. Try again.")
            isExtracting = false
            return
        end

        -- Throttle to avoid Roblox rate limits
        task.wait(0.15)
    end

    -- Finalize
    local finalizeSuccess, finalizeResult = httpPost("/extract/finalize", {
        project_dir = projectDir,
    })

    local elapsed = os.clock() - startTime
    if finalizeSuccess and finalizeResult.success then
        -- Save timing for future estimates
        lastExtractTime = elapsed
        lastInstanceCount = #allInstances
        local msg = string.format("Extracted %d instances (%.1fs)", #allInstances, elapsed)
        print(msg)
        setStatus(msg, Color3.fromRGB(100, 200, 100), 10)
    else
        local errMsg = "Extraction failed"
        warn(errMsg .. ". " .. (finalizeResult and finalizeResult.error or "Try again."))
        setStatus(errMsg, Color3.fromRGB(255, 100, 100), 10)
    end

    isExtracting = false
end

-- Sync: Create instance at path (silent)
local function syncCreate(payload: any): {success: boolean, error: string?}
    local path = payload.path
    local data = payload.data

    if not path or not data then
        print("[RbxSync Debug] syncCreate failed: Missing path or data")
        return { success = false, error = "Missing path or data" }
    end

    print("[RbxSync Debug] syncCreate: " .. path .. " (class=" .. tostring(data.className) .. ", name=" .. tostring(data.name) .. ")")

    -- Find parent (use '/' delimiter to handle instance names with periods)
    local pathParts = string.split(path, "/")
    table.remove(pathParts) -- Remove the instance name itself
    local parentPath = table.concat(pathParts, "/")

    local parent: Instance? = nil
    if #parentPath > 0 then
        parent = Sync.findInstanceAtPath(parentPath)
        if not parent then
            print("[RbxSync Debug] syncCreate failed: Parent not found at " .. parentPath)
            return { success = false, error = "Parent not found: " .. parentPath }
        end
    end

    -- Create the instance
    local instance = Sync.createInstance(data, parent)
    if instance then
        Sync.registerCreated(path, instance)
        print("[RbxSync Debug] syncCreate success: created " .. instance:GetFullName())
        return { success = true }
    else
        print("[RbxSync Debug] syncCreate failed: Sync.createInstance returned nil")
        return { success = false, error = "Can't create instance. Parent may be locked." }
    end
end

-- Sync: Update instance at path (silent)
-- Returns: success, error, wasSkipped (true if no changes needed)
local function syncUpdate(payload: any): {success: boolean, error: string?, skipped: boolean?}
    local path = payload.path
    local data = payload.data

    if not path or not data then
        return { success = false, error = "Missing path or data" }
    end

    local instance = Sync.findInstanceAtPath(path)
    if not instance then
        return syncCreate(payload)
    end

    -- If className changed, we need to replace the instance entirely
    if data.className and instance.ClassName ~= data.className then
        print("[RbxSync Debug] className changed from " .. instance.ClassName .. " to " .. data.className .. ", replacing instance")
        -- Get parent before destroying
        local parent = instance.Parent

        -- Remove old instance
        instance.Parent = nil

        -- Create new instance with correct className
        local newInstance = Sync.createInstance(data, parent)
        if newInstance then
            Sync.registerCreated(path, newInstance)
            return { success = true }
        else
            return { success = false, error = "Can't create replacement instance" }
        end
    end

    -- Check if update is actually needed (differential sync)
    local needsUpdate, reason = Sync.instanceNeedsUpdate(instance, data)
    if not needsUpdate then
        -- No changes needed - skip this update
        return { success = true, skipped = true }
    end

    Sync.registerCreated(path, instance)
    local ok = Sync.updateInstance(instance, data)
    if ok then
        return { success = true }
    else
        return { success = false, error = "Can't update instance" }
    end
end

-- Sync: Delete instance at path (silent)
local function syncDelete(payload: any): {success: boolean, error: string?}
    local path = payload.path

    if not path then
        return { success = false, error = "Missing path" }
    end

    local ok = Sync.deleteInstance(path)
    if ok then
        return { success = true }
    else
        return { success = false, error = "Instance not found" }
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

        -- For creates: shallow paths first (parents before children)
        -- For deletes: deep paths first (children before parents)
        if aType == "delete" then
            return aDepth > bDepth
        else
            return aDepth < bDepth
        end
    end)

    -- Clear cache before sync to handle undo/redo scenarios
    Sync.clearRecentlyCreated()

    isSyncing = true
    setStatus("Checking " .. #operations .. " instances...", Color3.fromRGB(100, 180, 255), 0)
    local results = {}
    local appliedCount = 0  -- Actually modified
    local skippedCount = 0  -- No changes needed
    local failCount = 0

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

    isSyncing = false

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
        -- Execute code with loadstring
        local fn, loadErr = loadstring(code)
        if not fn then
            return { success = false, error = "Syntax error: " .. tostring(loadErr), output = "" }
        end
        -- Replace print temporarily
        local env = setmetatable({ print = capturePrint }, { __index = getfenv() })
        setfenv(fn, env)
        local ok, runErr = pcall(fn)
        if not ok then
            return { success = false, error = "Runtime error: " .. tostring(runErr), output = table.concat(output, "\n") }
        end
        return { success = true, output = table.concat(output, "\n") }
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

                -- Check if this is a sync command and if files→studio sync is disabled
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
                                data = ok and result or nil,
                                error = (not ok and tostring(result)) or (result and result.error) or nil,
                            }
                            httpPost("/rbxsync/response", responseData)
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
    })

    if success and not hasLoggedLink then
        hasLoggedLink = true
        print(string.format("Linked to %s (%s)", projectDir, placeName))
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
extractButton.Text = "⬆  Extract"
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
    extractGame({})
    -- Reset studio changes after extract
    changeTracking.studioChanges = 0
    updateButtonBadges()
end)

-- Sync Button (Secondary - surface with stroke)
local syncButton = Instance.new("TextButton")
syncButton.Size = UDim2.new(0.5, -4, 1, 0)
syncButton.BackgroundColor3 = Colors.surface
syncButton.BorderSizePixel = 0
syncButton.Text = "⬇  Sync"
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
        -- Ready state: subtle secondary style
        syncButton.BackgroundColor3 = Colors.surface
        syncButton.TextColor3 = Colors.textPrimary
        syncButton.Text = "⬇  Sync"
        syncStroke.Color = Colors.textMuted
        syncStroke.Transparency = 0.8

    elseif state == SyncButtonState.SYNCING then
        -- Active syncing: accent color with pulse
        syncButton.BackgroundColor3 = Colors.accent
        syncButton.TextColor3 = Colors.background
        syncButton.Text = "⬇  Syncing..."
        syncStroke.Color = Colors.accent
        syncStroke.Transparency = 0

        -- Subtle pulse animation
        local pulseUp = true
        syncPulseConnection = game:GetService("RunService").Heartbeat:Connect(function()
            local current = syncButton.BackgroundTransparency
            if pulseUp then
                syncButton.BackgroundTransparency = math.min(current + 0.02, 0.3)
                if current >= 0.28 then pulseUp = false end
            else
                syncButton.BackgroundTransparency = math.max(current - 0.02, 0)
                if current <= 0.02 then pulseUp = true end
            end
        end)

    elseif state == SyncButtonState.DISCONNECTED then
        -- Disabled/disconnected: muted appearance
        syncButton.BackgroundColor3 = Colors.surface
        syncButton.TextColor3 = Colors.textMuted
        syncButton.Text = "⬇  Sync"
        syncStroke.Color = Colors.textMuted
        syncStroke.Transparency = 0.9
        syncButton.BackgroundTransparency = 0.5

    elseif state == SyncButtonState.SUCCESS then
        -- Brief success flash
        syncButton.BackgroundColor3 = Colors.success
        syncButton.TextColor3 = Colors.background
        syncButton.Text = "✓  Done"
        syncStroke.Color = Colors.success
        syncStroke.Transparency = 0

        -- Return to idle after 1.5s
        task.delay(1.5, function()
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
        extractButton.Text = "⬆ Extract (" .. changeTracking.studioChanges .. ")"
    else
        extractButton.Text = "⬆ Extract"
    end

    -- Update Sync button (only when idle)
    if currentSyncState == SyncButtonState.IDLE or currentSyncState == SyncButtonState.DISCONNECTED then
        if changeTracking.localChanges > 0 then
            syncButton.Text = "⬇ Sync (" .. changeTracking.localChanges .. ")"
        else
            syncButton.Text = "⬇ Sync"
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

    -- Sort by depth
    table.sort(operations, function(a, b)
        return select(2, string.gsub(a.path, "%.", "")) < select(2, string.gsub(b.path, "%.", ""))
    end)

    syncBatch({ operations = operations })

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

-- ============================================================================
-- Panel Button
-- ============================================================================

panelButton.Click:Connect(function()
    widget.Enabled = not widget.Enabled
    panelButton:SetActive(widget.Enabled)

    if widget.Enabled then
        task.spawn(updateInfoDisplay)
    end
end)

-- ============================================================================
-- Background Tasks
-- ============================================================================

task.spawn(pollLoop)

-- Connection check loop (respects userDisconnected flag)
task.spawn(function()
    while true do
        -- Only auto-reconnect if user hasn't explicitly disconnected
        if not userDisconnected then
            local wasConnected = isConnected
            local success, _ = httpGet("/health")

            -- Update connection state based on actual response
            if success ~= isConnected then
                isConnected = success
                if isConnected then
                    print("Connected")
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
            end
        else
            -- User disconnected - still check if server is actually available
            -- (so we can show accurate status if they want to reconnect)
            local success, _ = httpGet("/health")
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
