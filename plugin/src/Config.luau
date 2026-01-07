--!strict
--[[
    Config Module

    Manages plugin settings using Roblox PluginSettings API.
    Stores server URL, project directory, and recent projects.
]]

local HttpService = game:GetService("HttpService")

export type Settings = {
    serverUrl: string,
    projectDir: string,
    recentProjects: {string},
}

local Config = {}

-- Default settings
local defaults: Settings = {
    serverUrl = "http://localhost:44755",
    projectDir = "",
    recentProjects = {},
}
Config.defaults = defaults

-- Plugin reference (set during init)
local pluginRef: Plugin? = nil

-- Initialize with plugin reference
function Config.init(plugin: Plugin)
    pluginRef = plugin
end

-- Load settings from PluginSettings
function Config.load(): Settings
    if not pluginRef then
        warn("[RbxSync] Config not initialized with plugin reference")
        return Config.defaults
    end

    local serverUrl = pluginRef:GetSetting("RbxSync_ServerUrl")
    local projectDir = pluginRef:GetSetting("RbxSync_ProjectDir")
    local recentProjectsJson = pluginRef:GetSetting("RbxSync_RecentProjects")

    local recentProjects: {string} = {}
    if recentProjectsJson and #recentProjectsJson > 0 then
        local ok, decoded = pcall(function()
            return HttpService:JSONDecode(recentProjectsJson)
        end)
        if ok and type(decoded) == "table" then
            recentProjects = decoded :: {string}
        end
    end

    return {
        serverUrl = serverUrl or Config.defaults.serverUrl,
        projectDir = projectDir or Config.defaults.projectDir,
        recentProjects = recentProjects,
    }
end

-- Save settings to PluginSettings
function Config.save(settings: Settings)
    if not pluginRef then
        warn("[RbxSync] Config not initialized with plugin reference")
        return
    end

    pluginRef:SetSetting("RbxSync_ServerUrl", settings.serverUrl)
    pluginRef:SetSetting("RbxSync_ProjectDir", settings.projectDir)

    local recentProjectsJson = HttpService:JSONEncode(settings.recentProjects)
    pluginRef:SetSetting("RbxSync_RecentProjects", recentProjectsJson)
end

-- Add a project to recent projects list
function Config.addRecentProject(projectDir: string)
    local settings = Config.load()

    -- Remove if already exists
    local newRecent: {string} = {}
    for _, dir in settings.recentProjects do
        if dir ~= projectDir then
            table.insert(newRecent, dir)
        end
    end

    -- Add to front
    table.insert(newRecent, 1, projectDir)

    -- Keep only last 10
    while #newRecent > 10 do
        table.remove(newRecent)
    end

    settings.recentProjects = newRecent
    Config.save(settings)
end

-- Get current server URL
function Config.getServerUrl(): string
    return Config.load().serverUrl
end

-- Set server URL
function Config.setServerUrl(url: string)
    local settings = Config.load()
    settings.serverUrl = url
    Config.save(settings)
end

-- Get current project directory (per-PlaceId)
function Config.getProjectDir(): string
    if not pluginRef then
        warn("[RbxSync] Config not initialized with plugin reference")
        return ""
    end

    local placeId = tostring(game.PlaceId)
    if placeId == "0" then
        -- Unsaved place - use global default
        return pluginRef:GetSetting("RbxSync_ProjectDir") or ""
    end

    -- Per-place path, with fallback to global
    return pluginRef:GetSetting("RbxSync_ProjectDir_" .. placeId)
        or pluginRef:GetSetting("RbxSync_ProjectDir")
        or ""
end

-- Set project directory (per-PlaceId)
function Config.setProjectDir(dir: string)
    if not pluginRef then
        warn("[RbxSync] Config not initialized with plugin reference")
        return
    end

    local placeId = tostring(game.PlaceId)
    if placeId ~= "0" then
        -- Save per-place path
        pluginRef:SetSetting("RbxSync_ProjectDir_" .. placeId, dir)
    end

    -- Also save as global default for new/unsaved places
    pluginRef:SetSetting("RbxSync_ProjectDir", dir)
    Config.addRecentProject(dir)
end

return Config
