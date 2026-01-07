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

-- Configuration defaults
local POLL_INTERVAL = 1 -- seconds
local CHUNK_SIZE = 50 -- instances per chunk (Roblox HttpService has 1MB limit)

-- Load modules (modules are children of this script in the bundled .rbxm)
local Reflection = require(script.Reflection)
local Serializer = require(script.Serializer)
local Sync = require(script.Sync)
local Config = require(script.Config)
local Git = require(script.Git)
local TestRunner = require(script.TestRunner)

-- Initialize config with plugin reference
Config.init(plugin)

-- Plugin state
local isConnected = false
local isExtracting = false
local isSyncing = false

-- Get current server URL from config
local function getServerUrl(): string
    return Config.getServerUrl()
end

-- HTTP helpers
local function httpGet(endpoint: string): (boolean, any)
    local serverUrl = getServerUrl()
    local success, result = pcall(function()
        return HttpService:GetAsync(serverUrl .. endpoint)
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
        print(string.format("Extracted %d instances (%.1fs)", #allInstances, elapsed))
    else
        warn("Extraction failed. " .. (finalizeResult and finalizeResult.error or "Try again."))
    end

    isExtracting = false
end

-- Sync: Create instance at path (silent)
local function syncCreate(payload: any): {success: boolean, error: string?}
    local path = payload.path
    local data = payload.data

    if not path or not data then
        return { success = false, error = "Missing path or data" }
    end

    -- Find parent (use '/' delimiter to handle instance names with periods)
    local pathParts = string.split(path, "/")
    table.remove(pathParts) -- Remove the instance name itself
    local parentPath = table.concat(pathParts, "/")

    local parent: Instance? = nil
    if #parentPath > 0 then
        parent = Sync.findInstanceAtPath(parentPath)
        if not parent then
            return { success = false, error = "Parent not found" }
        end
    end

    -- Create the instance
    local instance = Sync.createInstance(data, parent)
    if instance then
        Sync.registerCreated(path, instance)
        return { success = true }
    else
        return { success = false, error = "Can't create instance. Parent may be locked." }
    end
end

-- Sync: Update instance at path (silent)
local function syncUpdate(payload: any): {success: boolean, error: string?}
    local path = payload.path
    local data = payload.data

    if not path or not data then
        return { success = false, error = "Missing path or data" }
    end

    local instance = Sync.findInstanceAtPath(path)
    if not instance then
        return syncCreate(payload)
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
local function syncBatch(payload: any): {success: boolean, results: {any}?, error: string?}
    local operations = payload.operations

    if not operations or type(operations) ~= "table" then
        return { success = false, error = "No operations to apply" }
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
    local results = {}
    local successCount = 0
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
            successCount += 1
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

    -- Summary
    if failCount > 0 then
        print(string.format("Synced %d/%d - %d failed (see warnings above)", successCount, #operations, failCount))
    else
        print(string.format("Synced %d changes", successCount))
    end

    return { success = failCount == 0, results = results }
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

            if success and response and response.command then
                task.spawn(function()
                    local recording = ChangeHistoryService:TryBeginRecording("RbxSync")

                    local ok, result = pcall(function()
                        return handleCommand(response.command, response.payload)
                    end)

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

-- Register with server (sends place info for multi-workspace support)
local function registerWithServer()
    local projectDir = Config.getProjectDir()
    if projectDir == "" then
        return false
    end

    local success, result = httpPost("/rbxsync/register", {
        place_id = game.PlaceId,
        place_name = game.Name or "Untitled",
        project_dir = projectDir,
    })

    if success then
        print(string.format("Linked to %s", projectDir))
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

-- UI State
local gitStatus: Git.GitStatus? = nil

-- Create main frame
local mainFrame = Instance.new("Frame")
mainFrame.Size = UDim2.new(1, 0, 1, 0)
mainFrame.BackgroundColor3 = Color3.fromRGB(46, 46, 46)
mainFrame.BorderSizePixel = 0
mainFrame.Parent = widget

local layout = Instance.new("UIListLayout")
layout.SortOrder = Enum.SortOrder.LayoutOrder
layout.Padding = UDim.new(0, 12)
layout.Parent = mainFrame

local padding = Instance.new("UIPadding")
padding.PaddingTop = UDim.new(0, 16)
padding.PaddingBottom = UDim.new(0, 16)
padding.PaddingLeft = UDim.new(0, 16)
padding.PaddingRight = UDim.new(0, 16)
padding.Parent = mainFrame

-- ============================================================================
-- Connection Status + Button
-- ============================================================================

local connectionFrame = Instance.new("Frame")
connectionFrame.Size = UDim2.new(1, 0, 0, 32)
connectionFrame.BackgroundTransparency = 1
connectionFrame.LayoutOrder = 1
connectionFrame.Parent = mainFrame

local connectionDot = Instance.new("Frame")
connectionDot.Size = UDim2.new(0, 10, 0, 10)
connectionDot.Position = UDim2.new(0, 0, 0.5, -5)
connectionDot.BackgroundColor3 = Color3.fromRGB(120, 120, 120)
connectionDot.Parent = connectionFrame

local dotCorner = Instance.new("UICorner")
dotCorner.CornerRadius = UDim.new(1, 0)
dotCorner.Parent = connectionDot

local connectionLabel = Instance.new("TextLabel")
connectionLabel.Size = UDim2.new(0, 100, 1, 0)
connectionLabel.Position = UDim2.new(0, 20, 0, 0)
connectionLabel.BackgroundTransparency = 1
connectionLabel.Text = "Disconnected"
connectionLabel.TextColor3 = Color3.fromRGB(180, 180, 180)
connectionLabel.TextSize = 14
connectionLabel.Font = Enum.Font.GothamMedium
connectionLabel.TextXAlignment = Enum.TextXAlignment.Left
connectionLabel.Parent = connectionFrame

local connectButton = Instance.new("TextButton")
connectButton.Size = UDim2.new(0, 80, 0, 28)
connectButton.Position = UDim2.new(1, -80, 0.5, -14)
connectButton.BackgroundColor3 = Color3.fromRGB(0, 120, 215)
connectButton.BorderSizePixel = 0
connectButton.Text = "Connect"
connectButton.TextColor3 = Color3.fromRGB(255, 255, 255)
connectButton.TextSize = 12
connectButton.Font = Enum.Font.GothamMedium
connectButton.AutoButtonColor = true
connectButton.Parent = connectionFrame

local connectBtnCorner = Instance.new("UICorner")
connectBtnCorner.CornerRadius = UDim.new(0, 4)
connectBtnCorner.Parent = connectButton

local function updateConnectionUI()
    if isConnected then
        connectionLabel.Text = "Connected"
        connectionLabel.TextColor3 = Color3.fromRGB(100, 200, 100)
        connectionDot.BackgroundColor3 = Color3.fromRGB(100, 200, 100)
        connectButton.Text = "Disconnect"
        connectButton.BackgroundColor3 = Color3.fromRGB(80, 80, 80)
    else
        connectionLabel.Text = "Disconnected"
        connectionLabel.TextColor3 = Color3.fromRGB(180, 180, 180)
        connectionDot.BackgroundColor3 = Color3.fromRGB(120, 120, 120)
        connectButton.Text = "Connect"
        connectButton.BackgroundColor3 = Color3.fromRGB(0, 120, 215)
    end
end

connectButton.MouseButton1Click:Connect(function()
    isConnected = not isConnected
    if isConnected then
        -- Check connection and register with server
        local success, _ = httpGet("/health")
        if success then
            registerWithServer()
        else
            isConnected = false  -- Connection failed
        end
    end
    updateConnectionUI()
end)

-- ============================================================================
-- Project Path
-- ============================================================================

local projectDirInput = Instance.new("TextBox")
projectDirInput.Size = UDim2.new(1, 0, 0, 32)
projectDirInput.BackgroundColor3 = Color3.fromRGB(60, 60, 60)
projectDirInput.BorderSizePixel = 0
projectDirInput.Text = Config.getProjectDir()
projectDirInput.PlaceholderText = "~/path/to/project"
projectDirInput.PlaceholderColor3 = Color3.fromRGB(100, 100, 100)
projectDirInput.TextColor3 = Color3.fromRGB(200, 200, 200)
projectDirInput.TextSize = 12
projectDirInput.Font = Enum.Font.Code
projectDirInput.TextXAlignment = Enum.TextXAlignment.Left
projectDirInput.LayoutOrder = 2
projectDirInput.ClearTextOnFocus = false
projectDirInput.Parent = mainFrame

local inputCorner = Instance.new("UICorner")
inputCorner.CornerRadius = UDim.new(0, 4)
inputCorner.Parent = projectDirInput

local inputPadding = Instance.new("UIPadding")
inputPadding.PaddingLeft = UDim.new(0, 10)
inputPadding.PaddingRight = UDim.new(0, 10)
inputPadding.Parent = projectDirInput

projectDirInput.FocusLost:Connect(function()
    if projectDirInput.Text ~= "" then
        Config.setProjectDir(projectDirInput.Text)
    end
end)

-- ============================================================================
-- Action Buttons (side by side)
-- ============================================================================

local actionsFrame = Instance.new("Frame")
actionsFrame.Size = UDim2.new(1, 0, 0, 36)
actionsFrame.BackgroundTransparency = 1
actionsFrame.LayoutOrder = 3
actionsFrame.Parent = mainFrame

local actionsLayout = Instance.new("UIListLayout")
actionsLayout.FillDirection = Enum.FillDirection.Horizontal
actionsLayout.SortOrder = Enum.SortOrder.LayoutOrder
actionsLayout.Padding = UDim.new(0, 8)
actionsLayout.Parent = actionsFrame

local function createActionButton(text: string, color: Color3, order: number, callback: () -> ()): TextButton
    local btn = Instance.new("TextButton")
    btn.Size = UDim2.new(0.5, -4, 1, 0)
    btn.BackgroundColor3 = color
    btn.BorderSizePixel = 0
    btn.Text = text
    btn.TextColor3 = Color3.fromRGB(255, 255, 255)
    btn.TextSize = 13
    btn.Font = Enum.Font.GothamMedium
    btn.LayoutOrder = order
    btn.AutoButtonColor = true

    local corner = Instance.new("UICorner")
    corner.CornerRadius = UDim.new(0, 4)
    corner.Parent = btn

    btn.MouseButton1Click:Connect(callback)
    return btn
end

local extractButton = createActionButton("Extract", Color3.fromRGB(0, 120, 215), 1, function()
    if not isConnected then
        warn("Not connected. Click Connect first.")
        return
    end
    if Config.getProjectDir() == "" then
        warn("No project set. Enter path above.")
        return
    end
    extractGame({})
end)
extractButton.Parent = actionsFrame

local syncButton = createActionButton("Sync", Color3.fromRGB(80, 80, 80), 2, function()
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

    -- Request file tree from server
    local success, result = httpPost("/sync/read-tree", {
        project_dir = projectDir,
    })

    if not success or not result.success then
        warn("Can't read files. " .. (result and result.error or "Try again."))
        return
    end

    local instances = result.instances
    if not instances or #instances == 0 then
        return -- Silent - nothing to sync
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
end)
syncButton.Parent = actionsFrame

-- ============================================================================
-- Git Status (one line)
-- ============================================================================

local gitLine = Instance.new("TextLabel")
gitLine.Size = UDim2.new(1, 0, 0, 20)
gitLine.BackgroundTransparency = 1
gitLine.Text = ""
gitLine.TextColor3 = Color3.fromRGB(140, 140, 140)
gitLine.TextSize = 12
gitLine.Font = Enum.Font.Gotham
gitLine.TextXAlignment = Enum.TextXAlignment.Left
gitLine.LayoutOrder = 4
gitLine.Parent = mainFrame

-- ============================================================================
-- Commit Input + Button
-- ============================================================================

local commitFrame = Instance.new("Frame")
commitFrame.Size = UDim2.new(1, 0, 0, 32)
commitFrame.BackgroundTransparency = 1
commitFrame.LayoutOrder = 5
commitFrame.Parent = mainFrame

local commitInput = Instance.new("TextBox")
commitInput.Size = UDim2.new(1, -70, 1, 0)
commitInput.BackgroundColor3 = Color3.fromRGB(60, 60, 60)
commitInput.BorderSizePixel = 0
commitInput.Text = ""
commitInput.PlaceholderText = "Commit message..."
commitInput.PlaceholderColor3 = Color3.fromRGB(100, 100, 100)
commitInput.TextColor3 = Color3.fromRGB(200, 200, 200)
commitInput.TextSize = 12
commitInput.Font = Enum.Font.Gotham
commitInput.TextXAlignment = Enum.TextXAlignment.Left
commitInput.ClearTextOnFocus = false
commitInput.Parent = commitFrame

local commitInputCorner = Instance.new("UICorner")
commitInputCorner.CornerRadius = UDim.new(0, 4)
commitInputCorner.Parent = commitInput

local commitInputPadding = Instance.new("UIPadding")
commitInputPadding.PaddingLeft = UDim.new(0, 10)
commitInputPadding.PaddingRight = UDim.new(0, 10)
commitInputPadding.Parent = commitInput

local commitButton = Instance.new("TextButton")
commitButton.Size = UDim2.new(0, 60, 1, 0)
commitButton.Position = UDim2.new(1, -60, 0, 0)
commitButton.BackgroundColor3 = Color3.fromRGB(40, 120, 40)
commitButton.BorderSizePixel = 0
commitButton.Text = "Commit"
commitButton.TextColor3 = Color3.fromRGB(255, 255, 255)
commitButton.TextSize = 12
commitButton.Font = Enum.Font.GothamMedium
commitButton.AutoButtonColor = true
commitButton.Parent = commitFrame

local commitBtnCorner = Instance.new("UICorner")
commitBtnCorner.CornerRadius = UDim.new(0, 4)
commitBtnCorner.Parent = commitButton

commitButton.MouseButton1Click:Connect(function()
    local message = commitInput.Text
    if message == "" then
        warn("Enter a commit message.")
        return
    end

    local success, err = Git.commit(message, true)
    if success then
        print("Committed: " .. message)
        commitInput.Text = ""
        task.spawn(updateGitDisplay)
    else
        warn("Commit failed. " .. (err or "Try again."))
    end
end)

-- Function to update git display
local function updateGitDisplay()
    local ok, status = Git.getStatus()
    if ok and status then
        gitStatus = status
        local changes = Git.getChangedCount(status)
        if changes > 0 then
            gitLine.Text = status.branch .. " - " .. changes .. " changed"
            gitLine.TextColor3 = Color3.fromRGB(255, 200, 100)
        else
            gitLine.Text = status.branch .. " - clean"
            gitLine.TextColor3 = Color3.fromRGB(100, 200, 100)
        end
    else
        gitLine.Text = "Not a git repo"
        gitLine.TextColor3 = Color3.fromRGB(140, 140, 140)
    end
end

-- ============================================================================
-- Panel Button
-- ============================================================================

panelButton.Click:Connect(function()
    widget.Enabled = not widget.Enabled
    panelButton:SetActive(widget.Enabled)

    if widget.Enabled then
        task.spawn(updateGitDisplay)
    end
end)

-- ============================================================================
-- Background Tasks
-- ============================================================================

task.spawn(pollLoop)

-- Connection check loop
task.spawn(function()
    while true do
        if isConnected then
            checkConnection()
            updateConnectionUI()
        end
        task.wait(5)
    end
end)

-- Git status refresh loop
task.spawn(function()
    while true do
        if widget.Enabled and Config.getProjectDir() ~= "" then
            updateGitDisplay()
        end
        task.wait(10)
    end
end)
