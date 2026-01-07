-- Reference to the RemoteEvent
local timeOverMinEvent = game.ReplicatedStorage:WaitForChild("TimeOverMinEvent")

-- Reference to your GUI element where you want to display timeOverMin
local timeOverMinLabel = script.Parent -- Assuming it's a child of the GUI

-- Listen for updates to timeOverMin from the server
timeOverMinEvent.OnClientEvent:Connect(function(timeOverMin)
	-- Update your GUI element with the new value
	timeOverMinLabel.Text = "Time Over Min: " .. tostring(timeOverMin)
end)
