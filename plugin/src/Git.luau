--!strict
--[[
    Git Module

    Client for git operations via the RbxSync server.
    Provides status, log, commit, and init functionality.
]]

local HttpService = game:GetService("HttpService")

local Config = require(script.Parent.Config)

export type GitStatus = {
    branch: string,
    is_dirty: boolean,
    staged_count: number,
    unstaged_count: number,
    untracked_count: number,
    ahead: number,
    behind: number,
}

export type GitCommit = {
    hash: string,
    message: string,
    author: string,
    date: string,
}

local Git = {}

-- HTTP helper
local function httpPost(endpoint: string, data: any): (boolean, any)
    local serverUrl = Config.getServerUrl()

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

-- Get git status for the current project
function Git.getStatus(): (boolean, GitStatus?, string?)
    local projectDir = Config.getProjectDir()

    if projectDir == "" then
        return false, nil, "No project set"
    end

    local ok, response = httpPost("/git/status", {
        project_dir = projectDir,
    })

    if not ok then
        return false, nil, "Server not responding"
    end

    if response.success then
        return true, response.data :: GitStatus, nil
    else
        return false, nil, response.error or "Unknown error"
    end
end

-- Get git commit history
function Git.getLog(limit: number?): (boolean, {GitCommit}?, string?)
    local projectDir = Config.getProjectDir()

    if projectDir == "" then
        return false, nil, "No project set"
    end

    local ok, response = httpPost("/git/log", {
        project_dir = projectDir,
        limit = limit or 5,
    })

    if not ok then
        return false, nil, "Server not responding"
    end

    if response.success then
        return true, response.data :: {GitCommit}, nil
    else
        return false, nil, response.error or "Unknown error"
    end
end

-- Commit all changes with a message
function Git.commit(message: string, addAll: boolean?): (boolean, string?)
    local projectDir = Config.getProjectDir()

    if projectDir == "" then
        return false, "No project set"
    end

    if message == "" then
        return false, "Enter a commit message"
    end

    local ok, response = httpPost("/git/commit", {
        project_dir = projectDir,
        message = message,
        add_all = addAll ~= false,
    })

    if not ok then
        return false, "Server not responding"
    end

    if response.success then
        return true, nil
    else
        return false, response.error or "Unknown error"
    end
end

-- Initialize a new git repository
function Git.init(): (boolean, string?)
    local projectDir = Config.getProjectDir()

    if projectDir == "" then
        return false, "No project set"
    end

    local ok, response = httpPost("/git/init", {
        project_dir = projectDir,
    })

    if not ok then
        return false, "Server not responding"
    end

    if response.success then
        return true, nil
    else
        return false, response.error or "Unknown error"
    end
end

-- Get a formatted status string for display
function Git.getStatusString(status: GitStatus): string
    local parts = {}

    if status.staged_count > 0 then
        table.insert(parts, status.staged_count .. " staged")
    end
    if status.unstaged_count > 0 then
        table.insert(parts, status.unstaged_count .. " modified")
    end
    if status.untracked_count > 0 then
        table.insert(parts, status.untracked_count .. " untracked")
    end

    if #parts == 0 then
        return "Clean"
    end

    return table.concat(parts, ", ")
end

-- Get total changed file count
function Git.getChangedCount(status: GitStatus): number
    return status.staged_count + status.unstaged_count + status.untracked_count
end

return Git
