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

-- Track connection state - don't queue changes when disconnected
local isConnectedToServer = false

-- Suppress outbound changes during inbound sync (prevents echo loops)
-- This is the primary mechanism to prevent data loss - see Azul plugin analysis
local isSyncingFromServer = false

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

-- Path-based deduplication: track recently applied paths with 50ms window (Azul pattern)
-- This provides responsive echo prevention without waiting for full grace period
local recentlyApplied: {[string]: number} = {}
local APPLY_DEBOUNCE = 0.05  -- 50ms window for path-based deduplication

-- Mark a specific path as recently synced from file watcher
function ChangeTracker.markPathFromFileWatcher(path: string)
    recentFileWatcherPaths[path] = tick()
end

-- Mark a path as recently applied from server (for 50ms deduplication window)
-- Call this when applying changes from sync to prevent echo back to server
function ChangeTracker.markPathApplied(path: string)
    recentlyApplied[path] = tick()
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

-- Set syncing from server state (suppresses ALL outbound changes during inbound sync)
-- This prevents echo loops: server→plugin→ChangeTracker→server→file→server...
-- Call with true before applying sync operations, false after completion
function ChangeTracker.setSyncingFromServer(syncing: boolean)
    isSyncingFromServer = syncing
    if syncing then
        -- Clear any pending changes to prevent stale echoes
        table.clear(pendingChanges)
    end
end

-- Check if currently syncing from server
function ChangeTracker.isSyncingFromServer(): boolean
    return isSyncingFromServer
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
    -- Primary echo prevention: suppress ALL changes during inbound sync
    if isSyncingFromServer then
        return
    end

    local isDelete = changeType == "delete"
    local path = getInstancePath(instance, isDelete)
    if not path then
        return
    end

    -- Path-based deduplication: skip if recently applied from server (50ms window)
    -- This is faster than the file watcher grace period for responsive echo prevention
    if recentlyApplied[path] and (tick() - recentlyApplied[path]) < APPLY_DEBOUNCE then
        return
    end

    -- Secondary echo prevention: grace period for file watcher paths
    -- Deletes always go through - they can't be echo from file watcher
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

    -- Don't attempt to send when not connected
    if not isConnectedToServer then
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
        -- Primary echo prevention: suppress ALL changes during inbound sync
        if isSyncingFromServer then
            return
        end

        -- Get the OLD path (stored before rename)
        local oldPath = instancePaths[instance]
        if not oldPath then
            -- Instance wasn't tracked, nothing to rename
            return
        end

        -- Calculate the NEW path by walking up the parent tree
        local parts = {}
        local current = instance
        while current and current ~= game do
            table.insert(parts, 1, current.Name)
            current = current.Parent
        end

        if #parts == 0 then
            return
        end

        -- Check if new path is in a tracked service
        local serviceName = parts[1]
        local isTracked = false
        for _, tracked in trackedServices do
            if tracked == serviceName then
                isTracked = true
                break
            end
        end

        if not isTracked then
            return
        end

        local newPath = table.concat(parts, "/")

        -- Skip if path didn't actually change (shouldn't happen, but be safe)
        if oldPath == newPath then
            return
        end

        -- Update the stored path immediately
        instancePaths[instance] = newPath

        -- Path-based deduplication: skip if recently applied from server (50ms window)
        if recentlyApplied[oldPath] and (tick() - recentlyApplied[oldPath]) < APPLY_DEBOUNCE then
            return
        end
        if recentlyApplied[newPath] and (tick() - recentlyApplied[newPath]) < APPLY_DEBOUNCE then
            return
        end

        -- Secondary echo prevention: grace period for file watcher paths
        if shouldIgnoreChange(oldPath) or shouldIgnoreChange(newPath) then
            return
        end

        -- Log the rename
        print("[RbxSync Debug] Queueing rename: " .. oldPath .. " -> " .. newPath)

        -- Queue rename operation (use newPath as key since that's the current identity)
        pendingChanges[newPath] = {
            path = newPath,
            className = instance.ClassName,
            changeType = "rename",
            data = {
                oldPath = oldPath,
                newPath = newPath,
            },
            timestamp = tick(),
        }

        -- Remove any pending changes for the old path (they're now stale)
        if pendingChanges[oldPath] then
            pendingChanges[oldPath] = nil
        end
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

-- Set connection state (controls whether changes are queued)
function ChangeTracker.setConnected(connected: boolean)
    isConnectedToServer = connected
    if connected then
        print("[RbxSync] Change tracking: connected")
    else
        print("[RbxSync] Change tracking: disconnected (changes will not be queued)")
    end
end

-- Check if connected
function ChangeTracker.isConnected(): boolean
    return isConnectedToServer
end

-- Get count of pending changes in queue
function ChangeTracker.getQueuedCount(): number
    local count = 0
    for _ in pairs(pendingChanges) do
        count = count + 1
    end
    return count
end

-- Clear all pending changes without sending
function ChangeTracker.clearQueue()
    local count = ChangeTracker.getQueuedCount()
    table.clear(pendingChanges)
    if count > 0 then
        print("[RbxSync] Cleared " .. count .. " queued change(s)")
    end
end

-- Force send all pending changes immediately (ignores debounce)
function ChangeTracker.flushQueue()
    if not isConnectedToServer then
        warn("[RbxSync] Cannot flush queue - not connected")
        return false
    end

    local count = ChangeTracker.getQueuedCount()
    if count == 0 then
        return true
    end

    -- Build request payload from all pending changes
    local operations = {}
    for path, change in pairs(pendingChanges) do
        table.insert(operations, {
            type = change.changeType,
            path = change.path,
            className = change.className,
            data = change.data,
        })
    end

    -- Clear queue before sending
    table.clear(pendingChanges)

    -- Send to server
    local HttpService = game:GetService("HttpService")
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
        print("[RbxSync] Synced " .. count .. " queued change(s)")
        return true
    else
        warn("[RbxSync] Failed to sync queued changes")
        return false
    end
end

return ChangeTracker
