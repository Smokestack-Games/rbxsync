--!strict
-- Service coverage shared by extraction, live tracking, and sync lookups

local Services = {}

Services.list = {
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
    "MaterialService",
}

Services.set = {} :: {[string]: boolean}
for _, name in Services.list do
    Services.set[name] = true
end

return Services
