--!strict
--[[
    Change Tracker Module

    Monitors changes made in Roblox Studio and reports them to the server
    for syncing back to the file system (two-way sync).
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

-- Pending changes to batch send (debounce)
local pendingChanges: {[string]: {
    path: string,
    className: string,
    changeType: string,  -- "create", "modify", "delete", "rename"
    data: any?,
    timestamp: number,
}} = {}

-- Last sync time to avoid echo (changes from file watcher)
local lastFileWatcherSync = 0
local FILE_WATCHER_GRACE_PERIOD = 2  -- seconds

-- Set last file watcher sync time (called when receiving sync from server)
function ChangeTracker.setLastFileWatcherSync()
    lastFileWatcherSync = tick()
end

-- Check if a change should be ignored (might be from file watcher)
local function shouldIgnoreChange(): boolean
    return (tick() - lastFileWatcherSync) < FILE_WATCHER_GRACE_PERIOD
end

-- Get the instance path relative to DataModel
local function getInstancePath(instance: Instance): string?
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

    return table.concat(parts, "/")
end

-- Queue a change for batched sending
local function queueChange(instance: Instance, changeType: string, data: any?)
    -- Skip if this might be from file watcher
    if shouldIgnoreChange() then
        return
    end

    local path = getInstancePath(instance)
    if not path then
        return
    end

    pendingChanges[path] = {
        path = path,
        className = instance.ClassName,
        changeType = changeType,
        data = data,
        timestamp = tick(),
    }
end

-- Send pending changes to server
local function sendPendingChanges()
    if not isTracking then
        return
    end

    -- Collect changes older than debounce period (300ms)
    local debounceTime = 0.3
    local now = tick()
    local readyChanges = {}

    for path, change in pairs(pendingChanges) do
        if now - change.timestamp >= debounceTime then
            table.insert(readyChanges, change)
            pendingChanges[path] = nil
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
        print("[RbxSync] Synced " .. #operations .. " change(s) to files")
    else
        -- Server might not have this endpoint yet, that's okay
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

        -- Serialize the changed property
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
    -- Track existing descendants
    for _, descendant in service:GetDescendants() do
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
    print("[RbxSync] Change tracking enabled (two-way sync)")

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

    print("[RbxSync] Change tracking disabled")
end

-- Check if tracking is active
function ChangeTracker.isTracking(): boolean
    return isTracking
end

return ChangeTracker
