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
        -- Try sending instances one at a time
        print("[RbxSync] Chunk too large, sending instances individually...")
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

    local projectDir = Config.getProjectDir()
    if projectDir == "" then
        warn("[RbxSync] No project directory configured")
        return
    end

    isExtracting = true
    local startTime = os.clock()

    print("[RbxSync] Starting game extraction to:", projectDir)

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

        local success = sendChunk(sessionId, chunkIndex, totalChunks, chunkInstances, projectDir)
        if not success then
            warn("[RbxSync] Extraction failed at chunk", chunkIndex + 1)
            isExtracting = false
            return
        end

        -- Throttle to avoid Roblox rate limits (~500 req/min)
        -- Wait 0.15s between chunks = ~400 req/min max
        task.wait(0.15)
    end

    -- Finalize: build proper file tree from chunks
    print("[RbxSync] Finalizing extraction...")
    local finalizeSuccess, finalizeResult = httpPost("/extract/finalize", {
        project_dir = projectDir,
    })

    if finalizeSuccess and finalizeResult.success then
        local elapsed = os.clock() - startTime
        print(string.format("[RbxSync] Extraction complete! %d instances, %d files, %d scripts in %.2fs",
            #allInstances,
            finalizeResult.filesWritten or 0,
            finalizeResult.scriptsWritten or 0,
            elapsed))
    else
        local elapsed = os.clock() - startTime
        warn("[RbxSync] Finalize failed:", finalizeResult and finalizeResult.error or "Unknown error")
        print(string.format("[RbxSync] Extraction sent %d instances in %.2fs (finalize pending)", #allInstances, elapsed))
    end

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
    -- Test runner commands for AI-powered development workflows
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
    -- Automated play test commands (using StudioTestService ExecutePlayModeAsync)
    elseif command == "test:run" then
        -- Start background test (non-blocking)
        -- payload: { duration?: number, mode?: "Play"|"Run" }
        return TestRunner.startBackgroundTest(payload)
    elseif command == "test:status" then
        -- Poll for test status and output
        return TestRunner.getTestStatus()
    elseif command == "test:finish" then
        -- Stop test and get final results
        return TestRunner.stopBackgroundTest()
    elseif command == "test:isRunning" then
        return { success = true, running = TestRunner.isRunning() }
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

-- ============================================================================
-- UI Creation
-- ============================================================================

-- Create toolbar
local toolbar = plugin:CreateToolbar("RbxSync")
local panelButton = toolbar:CreateButton(
    "RbxSync Panel",
    "Open RbxSync control panel",
    "rbxassetid://6031071053" -- Settings icon
)

-- Create dock widget
local widgetInfo = DockWidgetPluginGuiInfo.new(
    Enum.InitialDockState.Right,
    false,  -- Initially hidden
    false,  -- Override previous state
    300,    -- Default width
    500,    -- Default height
    250,    -- Min width
    300     -- Min height
)

local widget = plugin:CreateDockWidgetPluginGui("RbxSyncPanel", widgetInfo)
widget.Title = "RbxSync"

-- UI State
local gitStatus: Git.GitStatus? = nil
local gitCommits: {Git.GitCommit}? = nil

-- Create main frame
local mainFrame = Instance.new("ScrollingFrame")
mainFrame.Size = UDim2.new(1, 0, 1, 0)
mainFrame.BackgroundColor3 = Color3.fromRGB(46, 46, 46)
mainFrame.BorderSizePixel = 0
mainFrame.ScrollBarThickness = 6
mainFrame.CanvasSize = UDim2.new(0, 0, 0, 600)
mainFrame.Parent = widget

local layout = Instance.new("UIListLayout")
layout.SortOrder = Enum.SortOrder.LayoutOrder
layout.Padding = UDim.new(0, 8)
layout.Parent = mainFrame

local padding = Instance.new("UIPadding")
padding.PaddingTop = UDim.new(0, 12)
padding.PaddingBottom = UDim.new(0, 12)
padding.PaddingLeft = UDim.new(0, 12)
padding.PaddingRight = UDim.new(0, 12)
padding.Parent = mainFrame

-- Helper to create a section header
local function createSectionHeader(text: string, order: number): Frame
    local frame = Instance.new("Frame")
    frame.Size = UDim2.new(1, 0, 0, 24)
    frame.BackgroundTransparency = 1
    frame.LayoutOrder = order

    local label = Instance.new("TextLabel")
    label.Size = UDim2.new(1, 0, 1, 0)
    label.BackgroundTransparency = 1
    label.Text = text
    label.TextColor3 = Color3.fromRGB(200, 200, 200)
    label.TextSize = 14
    label.Font = Enum.Font.GothamBold
    label.TextXAlignment = Enum.TextXAlignment.Left
    label.Parent = frame

    return frame
end

-- Helper to create a button
local function createButton(text: string, order: number, callback: () -> ()): TextButton
    local button = Instance.new("TextButton")
    button.Size = UDim2.new(1, 0, 0, 32)
    button.BackgroundColor3 = Color3.fromRGB(0, 120, 215)
    button.BorderSizePixel = 0
    button.Text = text
    button.TextColor3 = Color3.fromRGB(255, 255, 255)
    button.TextSize = 14
    button.Font = Enum.Font.GothamMedium
    button.LayoutOrder = order
    button.AutoButtonColor = true

    local corner = Instance.new("UICorner")
    corner.CornerRadius = UDim.new(0, 4)
    corner.Parent = button

    button.MouseButton1Click:Connect(callback)
    return button
end

-- Helper to create a text input
local function createTextInput(placeholder: string, order: number): TextBox
    local textBox = Instance.new("TextBox")
    textBox.Size = UDim2.new(1, 0, 0, 32)
    textBox.BackgroundColor3 = Color3.fromRGB(60, 60, 60)
    textBox.BorderSizePixel = 0
    textBox.Text = ""
    textBox.PlaceholderText = placeholder
    textBox.PlaceholderColor3 = Color3.fromRGB(120, 120, 120)
    textBox.TextColor3 = Color3.fromRGB(255, 255, 255)
    textBox.TextSize = 13
    textBox.Font = Enum.Font.Gotham
    textBox.TextXAlignment = Enum.TextXAlignment.Left
    textBox.LayoutOrder = order
    textBox.ClearTextOnFocus = false

    local corner = Instance.new("UICorner")
    corner.CornerRadius = UDim.new(0, 4)
    corner.Parent = textBox

    local padding = Instance.new("UIPadding")
    padding.PaddingLeft = UDim.new(0, 8)
    padding.PaddingRight = UDim.new(0, 8)
    padding.Parent = textBox

    return textBox
end

-- Helper to create an info label
local function createInfoLabel(text: string, order: number): TextLabel
    local label = Instance.new("TextLabel")
    label.Size = UDim2.new(1, 0, 0, 20)
    label.BackgroundTransparency = 1
    label.Text = text
    label.TextColor3 = Color3.fromRGB(180, 180, 180)
    label.TextSize = 12
    label.Font = Enum.Font.Gotham
    label.TextXAlignment = Enum.TextXAlignment.Left
    label.LayoutOrder = order
    label.TextTruncate = Enum.TextTruncate.AtEnd
    return label
end

-- ============================================================================
-- Project Section
-- ============================================================================

createSectionHeader("PROJECT", 10).Parent = mainFrame

local projectDirLabel = createInfoLabel("No project selected", 11)
projectDirLabel.Parent = mainFrame

local projectDirInput = createTextInput("Enter project directory path...", 12)
projectDirInput.Text = Config.getProjectDir()
projectDirInput.Parent = mainFrame

projectDirInput.FocusLost:Connect(function(enterPressed)
    if projectDirInput.Text ~= "" then
        Config.setProjectDir(projectDirInput.Text)
        projectDirLabel.Text = projectDirInput.Text
        print("[RbxSync] Project directory set to:", projectDirInput.Text)
    end
end)

-- Update label with current project
local currentDir = Config.getProjectDir()
if currentDir ~= "" then
    projectDirLabel.Text = currentDir
end

-- ============================================================================
-- Connection Section
-- ============================================================================

createSectionHeader("CONNECTION", 20).Parent = mainFrame

local connectionLabel = createInfoLabel("Disconnected", 21)
connectionLabel.Parent = mainFrame

local connectButton: TextButton = nil :: any
connectButton = createButton("Connect", 22, function()
    isConnected = not isConnected

    if isConnected then
        print("[RbxSync] Connecting to server...")
        checkConnection()
        connectionLabel.Text = "Connected"
        connectionLabel.TextColor3 = Color3.fromRGB(100, 200, 100)
        connectButton.Text = "Disconnect"
    else
        print("[RbxSync] Disconnected")
        connectionLabel.Text = "Disconnected"
        connectionLabel.TextColor3 = Color3.fromRGB(180, 180, 180)
        connectButton.Text = "Connect"
    end
end)
connectButton.Parent = mainFrame

-- ============================================================================
-- Actions Section
-- ============================================================================

createSectionHeader("ACTIONS", 30).Parent = mainFrame

local extractButton = createButton("Extract Game", 31, function()
    if not isConnected then
        warn("[RbxSync] Not connected to server")
        return
    end

    local projectDir = Config.getProjectDir()
    if projectDir == "" then
        warn("[RbxSync] No project directory configured")
        return
    end

    extractGame({})
end)
extractButton.Parent = mainFrame

local syncButton = createButton("Sync to Studio", 32, function()
    if not isConnected then
        warn("[RbxSync] Not connected to server")
        return
    end

    local projectDir = Config.getProjectDir()
    if projectDir == "" then
        warn("[RbxSync] No project directory configured")
        return
    end

    if isSyncing then
        warn("[RbxSync] Sync already in progress")
        return
    end

    print("[RbxSync] Reading file tree from server...")

    -- Request file tree from server
    local success, result = httpPost("/sync/read-tree", {
        project_dir = projectDir,
    })

    if not success or not result.success then
        warn("[RbxSync] Failed to read file tree:", result and result.error or "Unknown error")
        return
    end

    local instances = result.instances
    if not instances or #instances == 0 then
        warn("[RbxSync] No instances found in project")
        return
    end

    print(string.format("[RbxSync] Loaded %d instances, starting sync...", #instances))

    -- Convert instances to update operations
    local operations = {}
    for _, inst in ipairs(instances) do
        local path = inst.path
        if path and path ~= "" then
            table.insert(operations, {
                type = "update",
                path = path,
                data = inst,
            })
        end
    end

    -- Sort by path depth (parents before children)
    table.sort(operations, function(a, b)
        local depthA = select(2, string.gsub(a.path, "%.", ""))
        local depthB = select(2, string.gsub(b.path, "%.", ""))
        return depthA < depthB
    end)

    -- Apply all operations
    local batchResult = syncBatch({ operations = operations })

    if batchResult.success then
        print("[RbxSync] Sync complete!")
    else
        warn("[RbxSync] Sync completed with errors")
    end
end)
syncButton.BackgroundColor3 = Color3.fromRGB(80, 80, 80)
syncButton.Parent = mainFrame

-- ============================================================================
-- Git Section
-- ============================================================================

createSectionHeader("GIT", 40).Parent = mainFrame

local gitBranchLabel = createInfoLabel("Branch: (not a git repo)", 41)
gitBranchLabel.Parent = mainFrame

local gitStatusLabel = createInfoLabel("Status: Unknown", 42)
gitStatusLabel.Parent = mainFrame

local commitMessageInput = createTextInput("Commit message...", 43)
commitMessageInput.Parent = mainFrame

local commitButton = createButton("Commit All Changes", 44, function()
    local message = commitMessageInput.Text
    if message == "" then
        warn("[RbxSync] Commit message cannot be empty")
        return
    end

    local success, err = Git.commit(message, true)
    if success then
        print("[RbxSync] Commit successful!")
        commitMessageInput.Text = ""
        -- Refresh git status
        task.spawn(function()
            task.wait(0.5)
            local ok, status = Git.getStatus()
            if ok and status then
                gitStatus = status
                gitBranchLabel.Text = "Branch: " .. status.branch
                gitStatusLabel.Text = "Status: " .. Git.getStatusString(status)
            end
        end)
    else
        warn("[RbxSync] Commit failed:", err or "Unknown error")
    end
end)
commitButton.BackgroundColor3 = Color3.fromRGB(40, 120, 40)
commitButton.Parent = mainFrame

createSectionHeader("RECENT COMMITS", 50).Parent = mainFrame

local commitsFrame = Instance.new("Frame")
commitsFrame.Size = UDim2.new(1, 0, 0, 80)
commitsFrame.BackgroundTransparency = 1
commitsFrame.LayoutOrder = 51
commitsFrame.Parent = mainFrame

local commitsLayout = Instance.new("UIListLayout")
commitsLayout.SortOrder = Enum.SortOrder.LayoutOrder
commitsLayout.Padding = UDim.new(0, 4)
commitsLayout.Parent = commitsFrame

-- Function to update git display
local function updateGitDisplay()
    -- Get status
    local ok, status = Git.getStatus()
    if ok and status then
        gitStatus = status
        gitBranchLabel.Text = "Branch: " .. status.branch
        gitStatusLabel.Text = "Status: " .. Git.getStatusString(status)

        if status.is_dirty then
            gitStatusLabel.TextColor3 = Color3.fromRGB(255, 200, 100)
        else
            gitStatusLabel.TextColor3 = Color3.fromRGB(100, 200, 100)
        end
    else
        gitBranchLabel.Text = "Branch: (not a git repo)"
        gitStatusLabel.Text = "Status: N/A"
        gitStatusLabel.TextColor3 = Color3.fromRGB(180, 180, 180)
    end

    -- Get commits
    local logOk, commits = Git.getLog(3)
    if logOk and commits then
        gitCommits = commits

        -- Clear existing commit labels
        for _, child in commitsFrame:GetChildren() do
            if child:IsA("TextLabel") then
                child:Destroy()
            end
        end

        -- Add commit labels
        for i, commit in commits do
            local commitLabel = Instance.new("TextLabel")
            commitLabel.Size = UDim2.new(1, 0, 0, 20)
            commitLabel.BackgroundTransparency = 1
            commitLabel.Text = commit.hash .. " - " .. commit.message
            commitLabel.TextColor3 = Color3.fromRGB(150, 150, 150)
            commitLabel.TextSize = 11
            commitLabel.Font = Enum.Font.Gotham
            commitLabel.TextXAlignment = Enum.TextXAlignment.Left
            commitLabel.TextTruncate = Enum.TextTruncate.AtEnd
            commitLabel.LayoutOrder = i
            commitLabel.Parent = commitsFrame
        end
    end
end

-- ============================================================================
-- Panel Button
-- ============================================================================

panelButton.Click:Connect(function()
    widget.Enabled = not widget.Enabled
    panelButton:SetActive(widget.Enabled)

    if widget.Enabled then
        -- Refresh git status when panel opens
        task.spawn(updateGitDisplay)
    end
end)

-- ============================================================================
-- Background Tasks
-- ============================================================================

-- Start polling loop
task.spawn(pollLoop)

-- Connection check loop
task.spawn(function()
    while true do
        if isConnected then
            checkConnection()
            if not isConnected then
                connectionLabel.Text = "Disconnected"
                connectionLabel.TextColor3 = Color3.fromRGB(180, 180, 180)
                connectButton.Text = "Connect"
            end
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
        task.wait(10) -- Refresh every 10 seconds
    end
end)

print("[RbxSync] Plugin loaded. Click toolbar button to open panel.")
