--!strict
--[[
    Change Tracker Module

    Monitors changes made in Roblox Studio and automatically extracts
    changes to the file system (auto-extract).
]]

local HttpService = game:GetService("HttpService")
local ScriptEditorService = game:GetService("ScriptEditorService")

local Config = require(script.Parent.Config)
local Serializer = require(script.Parent.Serializer)

local ChangeTracker = {}

-- Track whether we're actively monitoring
local isTracking = false

-- Services to track for changes
local trackedServices = {
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

-- Connections for cleanup
local connections: {RBXScriptConnection} = {}

-- Store instance paths for deletion (since parent is nil when DescendantRemoving fires)
local instancePaths: {[Instance]: string} = {}
setmetatable(instancePaths, { __mode = "k" })  -- Weak keys so instances can be GC'd

-- Pending changes to batch send (debounce)
local pendingChanges: {[string]: {
    path: string,
    className: string,
    changeType: string,  -- "create", "modify", "delete", "rename"
    data: any?,
    timestamp: number,
}} = {}

-- Track paths recently synced from file watcher to avoid echo
local recentFileWatcherPaths: {[string]: number} = {}
local FILE_WATCHER_GRACE_PERIOD = 0.5  -- seconds

-- Mark a specific path as recently synced from file watcher
function ChangeTracker.markPathFromFileWatcher(path: string)
    recentFileWatcherPaths[path] = tick()
end

-- Set last file watcher sync time (called when receiving sync from server)
-- Now accepts optional paths array to track specific changed paths
function ChangeTracker.setLastFileWatcherSync(paths: {string}?)
    local now = tick()
    if paths then
        for _, path in paths do
            recentFileWatcherPaths[path] = now
        end
    end
end

-- Clean up expired grace period entries
local function cleanupExpiredPaths()
    local now = tick()
    for path, timestamp in pairs(recentFileWatcherPaths) do
        if now - timestamp >= FILE_WATCHER_GRACE_PERIOD then
            recentFileWatcherPaths[path] = nil
        end
    end
end

-- Check if a change to a specific path should be ignored (might be from file watcher)
local function shouldIgnoreChange(path: string): boolean
    cleanupExpiredPaths()
    local timestamp = recentFileWatcherPaths[path]
    if timestamp and (tick() - timestamp) < FILE_WATCHER_GRACE_PERIOD then
        return true
    end
    return false
end

-- Get the instance path relative to DataModel
local function getInstancePath(instance: Instance, forDelete: boolean?): string?
    -- For deletes, use stored path since parent is nil when DescendantRemoving fires
    if forDelete and instancePaths[instance] then
        return instancePaths[instance]
    end

    local parts = {}
    local current = instance

    -- Walk up to find service
    while current and current ~= game do
        table.insert(parts, 1, current.Name)
        current = current.Parent
    end

    -- Must start with a tracked service
    if #parts == 0 then
        return nil
    end

    local serviceName = parts[1]
    local isTracked = false
    for _, tracked in trackedServices do
        if tracked == serviceName then
            isTracked = true
            break
        end
    end

    if not isTracked then
        return nil
    end

    local path = table.concat(parts, "/")

    -- Store the path for later (in case of deletion)
    instancePaths[instance] = path

    return path
end

-- Queue a change for batched sending (with deduplication)
local function queueChange(instance: Instance, changeType: string, data: any?)
    local isDelete = changeType == "delete"
    local path = getInstancePath(instance, isDelete)
    if not path then
        return
    end

    -- Deletes always go through - they can't be echo from file watcher
    -- Only check grace period for create/modify operations
    if changeType ~= "delete" and shouldIgnoreChange(path) then
        return
    end

    -- Check if we already have a pending change for this path
    local existing = pendingChanges[path]
    if existing then
        -- Update existing entry with new timestamp (reset debounce)
        -- For deletes, always update. For other changes, merge/update.
        if changeType == "delete" then
            existing.changeType = "delete"
            existing.data = nil
        else
            -- Keep the latest data, reset timestamp
            if data then
                existing.data = data
            end
        end
        existing.timestamp = tick()
        return  -- Don't log duplicate
    end

    -- New change - log and add to queue
    print("[RbxSync Debug] Queueing change: " .. changeType .. " " .. path)

    pendingChanges[path] = {
        path = path,
        className = instance.ClassName,
        changeType = changeType,
        data = data,
        timestamp = tick(),
    }
end

-- Configuration
local MAX_BATCH_SIZE = 100  -- Max changes per batch to prevent memory issues
local DEBOUNCE_TIME = 0.3   -- 300ms debounce

-- Send pending changes to server
local function sendPendingChanges()
    if not isTracking then
        return
    end

    -- Collect changes older than debounce period
    local now = tick()
    local readyChanges = {}

    for path, change in pairs(pendingChanges) do
        if now - change.timestamp >= DEBOUNCE_TIME then
            table.insert(readyChanges, change)
            pendingChanges[path] = nil

            -- Limit batch size to prevent memory issues
            if #readyChanges >= MAX_BATCH_SIZE then
                break
            end
        end
    end

    if #readyChanges == 0 then
        return
    end

    -- Build request payload
    local operations = {}
    for _, change in readyChanges do
        table.insert(operations, {
            type = change.changeType,
            path = change.path,
            className = change.className,
            data = change.data,
        })
    end

    -- Send to server
    local url = Config.getServerUrl() .. "/sync/from-studio"
    local payload = HttpService:JSONEncode({
        operations = operations,
        projectDir = Config.getProjectDir(),
    })

    local ok, result = pcall(function()
        return HttpService:RequestAsync({
            Url = url,
            Method = "POST",
            Headers = { ["Content-Type"] = "application/json" },
            Body = payload,
        })
    end)

    if ok and result.Success then
        -- Parse response to get detailed info
        local responseOk, responseData = pcall(function()
            return HttpService:JSONDecode(result.Body)
        end)

        if responseOk and responseData then
            local filesWritten = responseData.filesWritten or 0
            local errors = responseData.errors or {}

            if filesWritten > 0 then
                print("[RbxSync] Auto-extracted " .. filesWritten .. " file(s)")
            elseif #operations > 0 then
                warn("[RbxSync] Sync sent " .. #operations .. " changes but no files written - check project path")
            end

            if #errors > 0 then
                warn("[RbxSync] Sync errors:")
                for _, err in errors do
                    warn("  " .. err)
                end
            end
        else
            print("[RbxSync] Auto-extracted " .. #operations .. " change(s)")
        end
    elseif ok then
        warn("[RbxSync] Sync failed: HTTP " .. tostring(result.StatusCode))
    else
        warn("[RbxSync] Sync request failed")
    end
end

-- Track script source changes
local function trackScriptChanges(script: LuaSourceContainer)
    -- Use ScriptEditorService to detect when scripts are edited
    local connection = ScriptEditorService.TextDocumentDidClose:Connect(function(doc)
        if doc.Name == script:GetFullName() then
            -- Script was modified
            local source = ""
            pcall(function()
                source = (script :: any).Source
            end)

            queueChange(script, "modify", {
                properties = {
                    Source = { type = "string", value = source }
                }
            })
        end
    end)

    table.insert(connections, connection)
end

-- Track property changes on an instance
local function trackInstanceChanges(instance: Instance)
    -- Track property changes
    local propConnection = instance.Changed:Connect(function(property)
        -- Skip Name changes (handled separately)
        if property == "Name" then
            return
        end

        -- Skip Parent changes
        if property == "Parent" then
            return
        end

        -- Check if we already have a pending change - skip expensive serialization
        local path = getInstancePath(instance, false)
        if path and pendingChanges[path] then
            -- Just update timestamp to reset debounce
            pendingChanges[path].timestamp = tick()
            return
        end

        -- For scripts, track Source changes specially
        if instance:IsA("LuaSourceContainer") and property == "Source" then
            local source = ""
            pcall(function()
                source = (instance :: any).Source
            end)
            queueChange(instance, "modify", {
                properties = {
                    Source = { type = "string", value = source }
                }
            })
            return
        end

        -- Serialize the changed property (only if no pending change)
        local ok, serialized = pcall(function()
            return Serializer.serializeInstance(instance)
        end)

        if ok and serialized then
            queueChange(instance, "modify", serialized)
        end
    end)

    table.insert(connections, propConnection)

    -- Track name changes (renames)
    local nameConnection = instance:GetPropertyChangedSignal("Name"):Connect(function()
        -- TODO: Handle renames (need old path)
    end)

    table.insert(connections, nameConnection)
end

-- Set up tracking for a service
local function setupServiceTracking(service: Instance)
    -- Track existing descendants and store their paths
    for _, descendant in service:GetDescendants() do
        -- Store path for deletion tracking
        getInstancePath(descendant, false)
        trackInstanceChanges(descendant)
        if descendant:IsA("LuaSourceContainer") then
            trackScriptChanges(descendant)
        end
    end

    -- Track new instances
    local addedConnection = service.DescendantAdded:Connect(function(descendant)
        trackInstanceChanges(descendant)
        if descendant:IsA("LuaSourceContainer") then
            trackScriptChanges(descendant)
        end

        -- Queue create event
        local ok, serialized = pcall(function()
            return Serializer.serializeInstance(descendant)
        end)

        if ok and serialized then
            queueChange(descendant, "create", serialized)
        end
    end)

    table.insert(connections, addedConnection)

    -- Track removed instances
    local removedConnection = service.DescendantRemoving:Connect(function(descendant)
        print("[RbxSync Debug] DescendantRemoving fired: " .. descendant:GetFullName())
        queueChange(descendant, "delete", nil)
    end)

    table.insert(connections, removedConnection)
end

-- Start tracking changes
function ChangeTracker.start()
    if isTracking then
        return
    end

    isTracking = true
    print("[RbxSync] Auto-extract enabled")

    -- Set up tracking for all services
    for _, serviceName in trackedServices do
        local service = game:FindFirstChild(serviceName)
        if service then
            setupServiceTracking(service)
        end
    end

    -- Start background task to send changes
    task.spawn(function()
        while isTracking do
            sendPendingChanges()
            task.wait(0.5)  -- Check every 500ms
        end
    end)
end

-- Stop tracking changes
function ChangeTracker.stop()
    isTracking = false

    -- Disconnect all connections
    for _, connection in connections do
        connection:Disconnect()
    end
    table.clear(connections)
    table.clear(pendingChanges)

    print("[RbxSync] Auto-extract disabled")
end

-- Check if tracking is active
function ChangeTracker.isTracking(): boolean
    return isTracking
end

return ChangeTracker
