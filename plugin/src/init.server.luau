--!strict
--[[
    RbxSync Studio Plugin

    Extracts complete game DataModel with ALL properties using reflection.
    Communicates with local RbxSync server via HTTP.
]]

local HttpService = game:GetService("HttpService")
local Selection = game:GetService("Selection")
local ChangeHistoryService = game:GetService("ChangeHistoryService")

-- Configuration
local SERVER_URL = "http://localhost:44755"
local POLL_INTERVAL = 1 -- seconds
local CHUNK_SIZE = 50 -- instances per chunk (Roblox HttpService has 1MB limit)

-- Load modules (modules are children of this script in the bundled .rbxm)
local Reflection = require(script.Reflection)
local Serializer = require(script.Serializer)
local Sync = require(script.Sync)

-- Plugin state
local isConnected = false
local isExtracting = false
local isSyncing = false

-- HTTP helpers
local function httpGet(endpoint: string): (boolean, any)
    local success, result = pcall(function()
        return HttpService:GetAsync(SERVER_URL .. endpoint)
    end)

    if success then
        local ok, data = pcall(function()
            return HttpService:JSONDecode(result)
        end)
        return ok, data
    end

    return false, result
end

local function httpPost(endpoint: string, data: any): (boolean, any)
    local success, result = pcall(function()
        local json = HttpService:JSONEncode(data)
        return HttpService:PostAsync(SERVER_URL .. endpoint, json, Enum.HttpContentType.ApplicationJson)
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
local function sendChunk(sessionId: string, chunkIndex: number, totalChunks: number, instances: {any}): boolean
    local success, result = httpPost("/extract/chunk", {
        session_id = sessionId,
        chunk_index = chunkIndex,
        total_chunks = totalChunks,
        data = instances,
    })

    if success then
        return true
    end

    -- Check if it's a size limit error
    if type(result) == "string" and result:find("too large") then
        -- Try sending instances one at a time
        print("[RbxSync] Chunk too large, sending instances individually...")
        for i, inst in ipairs(instances) do
            local singleSuccess, singleResult = httpPost("/extract/chunk", {
                session_id = sessionId,
                chunk_index = chunkIndex,
                total_chunks = totalChunks,
                data = {inst},
            })
            if not singleSuccess then
                if type(singleResult) == "string" and singleResult:find("too large") then
                    warn("[RbxSync] Skipping oversized instance:", inst.name or "unknown", inst.className or "unknown")
                else
                    warn("[RbxSync] Failed to send instance:", singleResult)
                    return false
                end
            end
        end
        return true
    end

    warn("[RbxSync] Failed to send chunk:", result)
    return false
end

-- Extract full game
local function extractGame(config: {any})
    if isExtracting then
        warn("[RbxSync] Extraction already in progress")
        return
    end

    isExtracting = true
    local startTime = os.clock()

    print("[RbxSync] Starting game extraction...")

    -- Initialize reflection engine with API dump
    local apiDump = Reflection.loadAPIDump()
    if not apiDump then
        warn("[RbxSync] Failed to load API dump")
        isExtracting = false
        return
    end

    print("[RbxSync] Loaded API dump with", #apiDump.Classes, "classes")

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

    print("[RbxSync] Found", #allInstances, "instances to extract")

    -- Calculate chunks
    local totalChunks = math.ceil(#allInstances / CHUNK_SIZE)
    local sessionId = HttpService:GenerateGUID(false)

    -- Serialize and send in chunks
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

        print(string.format("[RbxSync] Sending chunk %d/%d (%d instances)",
            chunkIndex + 1, totalChunks, #chunkInstances))

        local success = sendChunk(sessionId, chunkIndex, totalChunks, chunkInstances)
        if not success then
            warn("[RbxSync] Extraction failed at chunk", chunkIndex + 1)
            isExtracting = false
            return
        end

        -- Throttle to avoid Roblox rate limits (~500 req/min)
        -- Wait 0.15s between chunks = ~400 req/min max
        task.wait(0.15)
    end

    local elapsed = os.clock() - startTime
    print(string.format("[RbxSync] Extraction complete! %d instances in %.2fs", #allInstances, elapsed))

    isExtracting = false
end

-- Sync: Create instance at path
local function syncCreate(payload: any): {success: boolean, error: string?}
    local path = payload.path
    local data = payload.data

    if not path or not data then
        return { success = false, error = "Missing path or data" }
    end

    -- Find parent
    local pathParts = string.split(path, ".")
    table.remove(pathParts) -- Remove the instance name itself
    local parentPath = table.concat(pathParts, ".")

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
        -- Register for path lookup (handles name mismatches)
        Sync.registerCreated(path, instance)
        print("[RbxSync] Created:", path)
        return { success = true }
    else
        return { success = false, error = "Failed to create instance" }
    end
end

-- Sync: Update instance at path
local function syncUpdate(payload: any): {success: boolean, error: string?}
    local path = payload.path
    local data = payload.data

    if not path or not data then
        return { success = false, error = "Missing path or data" }
    end

    local instance = Sync.findInstanceAtPath(path)
    if not instance then
        -- Instance doesn't exist, try to create it
        local result = syncCreate(payload)
        return result
    end

    -- Register existing instance for path lookup
    Sync.registerCreated(path, instance)

    local ok = Sync.updateInstance(instance, data)
    if ok then
        print("[RbxSync] Updated:", path)
        return { success = true }
    else
        return { success = false, error = "Failed to update instance" }
    end
end

-- Sync: Delete instance at path
local function syncDelete(payload: any): {success: boolean, error: string?}
    local path = payload.path

    if not path then
        return { success = false, error = "Missing path" }
    end

    local ok = Sync.deleteInstance(path)
    if ok then
        print("[RbxSync] Deleted:", path)
        return { success = true }
    else
        return { success = false, error = "Instance not found: " .. path }
    end
end

-- Sync: Apply batch of operations
local function syncBatch(payload: any): {success: boolean, results: {any}?, error: string?}
    local operations = payload.operations

    if not operations or type(operations) ~= "table" then
        return { success = false, error = "Missing or invalid operations" }
    end

    isSyncing = true
    local results = {}
    local successCount = 0
    local failCount = 0

    print(string.format("[RbxSync] Applying %d operations...", #operations))

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
            result = { success = false, error = "Unknown operation: " .. tostring(opType) }
        end

        table.insert(results, result)

        if result.success then
            successCount += 1
        else
            failCount += 1
            warn("[RbxSync] Failed:", op.path or "unknown", "-", result.error or "unknown error")
        end

        -- Yield occasionally to prevent freezing
        if i % 100 == 0 then
            task.wait()
            print(string.format("[RbxSync] Progress: %d/%d", i, #operations))
        end
    end

    isSyncing = false
    print(string.format("[RbxSync] Sync complete: %d succeeded, %d failed", successCount, failCount))

    return { success = failCount == 0, results = results }
end

-- Handle incoming commands from server
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
        print("[RbxSync] Received ping")
        return { success = true }
    else
        warn("[RbxSync] Unknown command:", command)
        return { success = false, error = "Unknown command: " .. command }
    end
end

-- Main polling loop
local function pollLoop()
    while true do
        if isConnected then
            local success, response = httpGet("/rbxsync/request")

            if success and response and response.command then
                print("[RbxSync] Received command:", response.command)

                task.spawn(function()
                    local recording = ChangeHistoryService:TryBeginRecording("RbxSync")

                    local ok, result = pcall(function()
                        return handleCommand(response.command, response.payload)
                    end)

                    if not ok then
                        warn("[RbxSync] Command error:", result)
                    end

                    if recording then
                        ChangeHistoryService:FinishRecording(recording, Enum.FinishRecordingOperation.Commit)
                    end

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

        task.wait(POLL_INTERVAL)
    end
end

-- Check server connection
local function checkConnection()
    local success, _ = httpGet("/health")

    if success ~= isConnected then
        isConnected = success
        if isConnected then
            print("[RbxSync] Connected to server")
        else
            print("[RbxSync] Disconnected from server")
        end
    end
end

-- Create toolbar button
local toolbar = plugin:CreateToolbar("RbxSync")
local toggleButton = toolbar:CreateButton(
    "Toggle RbxSync",
    "Connect/disconnect from RbxSync server",
    "" -- No icon (uses default)
)

local extractButton = toolbar:CreateButton(
    "Extract Game",
    "Extract current game to files",
    "" -- No icon (uses default)
)

toggleButton.Click:Connect(function()
    isConnected = not isConnected
    toggleButton:SetActive(isConnected)

    if isConnected then
        print("[RbxSync] Enabled - connecting to server...")
        checkConnection()
    else
        print("[RbxSync] Disabled")
    end
end)

extractButton.Click:Connect(function()
    if not isConnected then
        warn("[RbxSync] Not connected to server. Click Toggle button first.")
        return
    end

    extractGame({})
end)

-- Start background tasks
task.spawn(pollLoop)
task.spawn(function()
    while true do
        if isConnected then
            checkConnection()
        end
        task.wait(5)
    end
end)

print("[RbxSync] Plugin loaded. Click toolbar button to connect.")
