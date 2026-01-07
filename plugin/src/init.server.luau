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
local userDisconnected = false  -- True when user explicitly clicks Disconnect (prevents auto-reconnect)
local cachedPlaceName: string? = nil
local lastExtractTime: number? = nil  -- Track last extraction time for estimates
local lastInstanceCount: number? = nil  -- Track last instance count

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

-- Count total instances in standard services (for estimates)
local function countInstances(): number
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
        "Teams",
        "Chat",
        "LocalizationService",
        "TestService",
    }

    local count = 0
    for _, serviceName in servicesToCount do
        local service = game:FindFirstChild(serviceName)
        if service then
            count += 1 + #service:GetDescendants()
        end
    end
    return count
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
    setStatus("Syncing " .. #operations .. " changes...", Color3.fromRGB(100, 180, 255), 0)
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
        local msg = string.format("Synced %d/%d - %d failed", successCount, #operations, failCount)
        print(msg .. " (see warnings above)")
        setStatus(msg, Color3.fromRGB(255, 200, 100), 10)
    else
        local msg = string.format("Synced %d changes", successCount)
        print(msg)
        setStatus(msg, Color3.fromRGB(100, 200, 100), 10)
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

            -- Detect disconnection from failed request
            if not success then
                isConnected = false
                print("Disconnected")
                updateConnectionUI()
            elseif response and response.command then
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

    local placeName = getPlaceName()
    local success, result = httpPost("/rbxsync/register", {
        place_id = game.PlaceId,
        place_name = placeName,
        project_dir = projectDir,
    })

    if success then
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

updateConnectionUI = function()
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
    if isConnected then
        -- User clicked Disconnect
        isConnected = false
        userDisconnected = true  -- Prevent auto-reconnect
        print("Disconnected by user")
    else
        -- User clicked Connect
        userDisconnected = false  -- Allow reconnection
        local success, _ = httpGet("/health")
        if success then
            isConnected = true
            registerWithServer()
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

local projectPathFrame = Instance.new("Frame")
projectPathFrame.Size = UDim2.new(1, 0, 0, 32)
projectPathFrame.BackgroundTransparency = 1
projectPathFrame.LayoutOrder = 2
projectPathFrame.Parent = mainFrame

local projectDirInput = Instance.new("TextBox")
projectDirInput.Size = UDim2.new(1, -36, 1, 0)
projectDirInput.BackgroundColor3 = Color3.fromRGB(60, 60, 60)
projectDirInput.BorderSizePixel = 0
projectDirInput.Text = Config.getProjectDir()
projectDirInput.PlaceholderText = "~/path/to/project"
projectDirInput.PlaceholderColor3 = Color3.fromRGB(100, 100, 100)
projectDirInput.TextColor3 = Color3.fromRGB(200, 200, 200)
projectDirInput.TextSize = 12
projectDirInput.Font = Enum.Font.Code
projectDirInput.TextXAlignment = Enum.TextXAlignment.Left
projectDirInput.ClearTextOnFocus = false
projectDirInput.Parent = projectPathFrame

local inputCorner = Instance.new("UICorner")
inputCorner.CornerRadius = UDim.new(0, 4)
inputCorner.Parent = projectDirInput

local inputPadding = Instance.new("UIPadding")
inputPadding.PaddingLeft = UDim.new(0, 10)
inputPadding.PaddingRight = UDim.new(0, 10)
inputPadding.Parent = projectDirInput

-- Dropdown toggle button
local dropdownButton = Instance.new("TextButton")
dropdownButton.Size = UDim2.new(0, 32, 1, 0)
dropdownButton.Position = UDim2.new(1, -32, 0, 0)
dropdownButton.BackgroundColor3 = Color3.fromRGB(70, 70, 70)
dropdownButton.BorderSizePixel = 0
dropdownButton.Text = "▼"
dropdownButton.TextColor3 = Color3.fromRGB(180, 180, 180)
dropdownButton.TextSize = 10
dropdownButton.Font = Enum.Font.GothamBold
dropdownButton.AutoButtonColor = true
dropdownButton.Parent = projectPathFrame

local dropdownBtnCorner = Instance.new("UICorner")
dropdownBtnCorner.CornerRadius = UDim.new(0, 4)
dropdownBtnCorner.Parent = dropdownButton

-- Dropdown menu frame (hidden by default)
local dropdownFrame = Instance.new("Frame")
dropdownFrame.Size = UDim2.new(1, 0, 0, 0) -- Height set dynamically
dropdownFrame.Position = UDim2.new(0, 0, 1, 4)
dropdownFrame.BackgroundColor3 = Color3.fromRGB(50, 50, 50)
dropdownFrame.BorderSizePixel = 0
dropdownFrame.Visible = false
dropdownFrame.ZIndex = 10
dropdownFrame.Parent = projectPathFrame

local dropdownCorner = Instance.new("UICorner")
dropdownCorner.CornerRadius = UDim.new(0, 4)
dropdownCorner.Parent = dropdownFrame

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
        noProjects.TextColor3 = Color3.fromRGB(120, 120, 120)
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
        local option = Instance.new("TextButton")
        option.Size = UDim2.new(1, -8, 0, 24)
        option.BackgroundColor3 = projectPath == currentPath and Color3.fromRGB(0, 100, 180) or Color3.fromRGB(60, 60, 60)
        option.BorderSizePixel = 0
        option.Text = "  " .. projectPath
        option.TextColor3 = projectPath == currentPath and Color3.fromRGB(255, 255, 255) or Color3.fromRGB(180, 180, 180)
        option.TextSize = 11
        option.Font = Enum.Font.Code
        option.TextXAlignment = Enum.TextXAlignment.Left
        option.TextTruncate = Enum.TextTruncate.AtEnd
        option.AutoButtonColor = true
        option.LayoutOrder = i
        option.ZIndex = 11
        option.Parent = dropdownFrame

        local optionCorner = Instance.new("UICorner")
        optionCorner.CornerRadius = UDim.new(0, 3)
        optionCorner.Parent = option

        option.MouseButton1Click:Connect(function()
            Config.setProjectDir(projectPath)
            projectDirInput.Text = projectPath
            dropdownFrame.Visible = false
            dropdownButton.Text = "▼"
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
    dropdownButton.Text = dropdownFrame.Visible and "▲" or "▼"
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
-- Project Info Section
-- ============================================================================

local infoFrame = Instance.new("Frame")
infoFrame.Size = UDim2.new(1, 0, 0, 66)  -- Taller to fit status line
infoFrame.BackgroundColor3 = Color3.fromRGB(55, 55, 55)
infoFrame.BorderSizePixel = 0
infoFrame.LayoutOrder = 4
infoFrame.Parent = mainFrame

local infoFrameCorner = Instance.new("UICorner")
infoFrameCorner.CornerRadius = UDim.new(0, 4)
infoFrameCorner.Parent = infoFrame

local infoLayout = Instance.new("UIListLayout")
infoLayout.SortOrder = Enum.SortOrder.LayoutOrder
infoLayout.Padding = UDim.new(0, 2)
infoLayout.Parent = infoFrame

local infoPadding = Instance.new("UIPadding")
infoPadding.PaddingTop = UDim.new(0, 6)
infoPadding.PaddingLeft = UDim.new(0, 8)
infoPadding.PaddingRight = UDim.new(0, 8)
infoPadding.Parent = infoFrame

-- Place name line
local placeNameLine = Instance.new("TextLabel")
placeNameLine.Size = UDim2.new(1, 0, 0, 16)
placeNameLine.BackgroundTransparency = 1
placeNameLine.Text = "Loading..."
placeNameLine.TextColor3 = Color3.fromRGB(200, 200, 200)
placeNameLine.TextSize = 12
placeNameLine.Font = Enum.Font.GothamMedium
placeNameLine.TextXAlignment = Enum.TextXAlignment.Left
placeNameLine.LayoutOrder = 1
placeNameLine.Parent = infoFrame

-- Instance count line
local instanceLine = Instance.new("TextLabel")
instanceLine.Size = UDim2.new(1, 0, 0, 14)
instanceLine.BackgroundTransparency = 1
instanceLine.Text = "..."
instanceLine.TextColor3 = Color3.fromRGB(140, 140, 140)
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
statusLine.TextColor3 = Color3.fromRGB(100, 180, 255)
statusLine.TextSize = 11
statusLine.Font = Enum.Font.GothamMedium
statusLine.TextXAlignment = Enum.TextXAlignment.Left
statusLine.LayoutOrder = 3
statusLine.Parent = infoFrame

-- Status update function (updates forward declaration)
local statusClearTimeout: thread? = nil
setStatus = function(text: string, color: Color3?, duration: number?)
    statusLine.Text = text
    statusLine.TextColor3 = color or Color3.fromRGB(100, 180, 255)

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

-- ============================================================================
-- Git Status Section
-- ============================================================================

local gitLine = Instance.new("TextLabel")
gitLine.Size = UDim2.new(1, 0, 0, 20)
gitLine.BackgroundTransparency = 1
gitLine.Text = ""
gitLine.TextColor3 = Color3.fromRGB(140, 140, 140)
gitLine.TextSize = 12
gitLine.Font = Enum.Font.Gotham
gitLine.TextXAlignment = Enum.TextXAlignment.Left
gitLine.LayoutOrder = 5
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

    -- Update instance count and estimate
    instanceLine.Text = getExtractionEstimate()
end

-- Function to update git display
local function updateGitDisplay()
    local ok, status = Git.getStatus()
    if ok and status then
        gitStatus = status
        local changes = Git.getChangedCount(status)
        local branchInfo = status.branch or "unknown"

        -- Add ahead/behind info if available
        if status.ahead and status.ahead > 0 then
            branchInfo = branchInfo .. " ↑" .. status.ahead
        end
        if status.behind and status.behind > 0 then
            branchInfo = branchInfo .. " ↓" .. status.behind
        end

        if changes > 0 then
            gitLine.Text = branchInfo .. " • " .. changes .. " changed"
            gitLine.TextColor3 = Color3.fromRGB(255, 200, 100)
        else
            gitLine.Text = branchInfo .. " • clean"
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
        task.spawn(updateInfoDisplay)
        task.spawn(updateGitDisplay)
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
            checkConnection()
            -- Update UI if state changed
            if wasConnected ~= isConnected then
                updateConnectionUI()
            end
        end
        task.wait(5)
    end
end)

-- Info and git status refresh loop
task.spawn(function()
    -- Initial update
    task.wait(1)
    updateInfoDisplay()

    while true do
        if widget.Enabled then
            updateInfoDisplay()
            if Config.getProjectDir() ~= "" then
                updateGitDisplay()
            end
        end
        task.wait(10)
    end
end)
