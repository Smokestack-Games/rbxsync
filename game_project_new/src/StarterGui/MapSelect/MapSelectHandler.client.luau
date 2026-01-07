-- Variables
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local PlayerJoinQueue = ReplicatedStorage:WaitForChild("PlayerJoinQueue")
local PlayerLeaveQueue = ReplicatedStorage:WaitForChild("PlayerLeaveQueue")
local UpdateQueueInfo = ReplicatedStorage:WaitForChild("UpdateQueueInfo")
local Player = game.Players.LocalPlayer

local ScreenGui = script.Parent -- Assuming this script is in the ScreenGui.
local FrameTemplate = script:WaitForChild("Template")
local GridLayout = ScreenGui:WaitForChild("Horizontal")

local playerInQueue = nil -- Track if the player is in any queue

-- Function to handle frame clicked (joining/leaving queue)
local function handleFrameClicked(frame, index)
	local queueButton = frame:WaitForChild("QueueButton")
	queueButton.MouseButton1Click:Connect(function()
		if playerInQueue == index then
			PlayerLeaveQueue:InvokeServer(index)
			GridLayout["Frame" .. playerInQueue].QueueButton.UIStroke.Color = Color3.new(255, 255, 255)
			GridLayout["Frame" .. playerInQueue].QueueButton.UIStroke.Thickness = 2
			playerInQueue = nil
			frame.QueueButton.ButtonFrame.Visible = false
		else
			if playerInQueue ~= nil then
				PlayerLeaveQueue:InvokeServer(playerInQueue)
				GridLayout["Frame" .. playerInQueue].QueueButton.ButtonFrame.Visible = false
				GridLayout["Frame" .. playerInQueue].QueueButton.UIStroke.Color = Color3.new(255, 255, 255)
				GridLayout["Frame" .. playerInQueue].QueueButton.UIStroke.Thickness = 2
				playerInQueue = nil	
			end
			PlayerJoinQueue:InvokeServer(index)
			playerInQueue = index
			frame.QueueButton.ButtonFrame.Visible = true
			GridLayout["Frame" .. playerInQueue].QueueButton.UIStroke.Color = Color3.new(0, 249, 0)
			GridLayout["Frame" .. playerInQueue].QueueButton.UIStroke.Thickness = 4
		end
	end)
	
	queueButton.ButtonFrame.LeaveQueue.MouseButton1Click:Connect(function()
		if playerInQueue then
			PlayerLeaveQueue:InvokeServer(playerInQueue)
			GridLayout["Frame" .. playerInQueue].QueueButton.ButtonFrame.Visible = false
			GridLayout["Frame" .. playerInQueue].QueueButton.UIStroke.Color = Color3.new(255, 255, 255)
			GridLayout["Frame" .. playerInQueue].QueueButton.UIStroke.Thickness = 2
		end
	end)
end

local function createOrUpdateFrame(index, playerCount, timeLeft, map, gameMode)
	local frame = GridLayout:FindFirstChild("Frame" .. index) or FrameTemplate:Clone()
	frame.Name = "Frame" .. index
	--print(gameMode)
	--print(map)
	frame.QueueButton.Gamemode.Text = gameMode
	frame.QueueButton.Image = map
	frame.QueueButton.Bottom.Players.Text = playerCount
	frame.QueueButton.Bottom.Timer.Text = tostring(timeLeft)
	if timeLeft == 60 then
		if playerInQueue == index then
			task.spawn(function()
				GridLayout["Frame" .. playerInQueue].QueueButton.ButtonFrame.Visible = false
				GridLayout["Frame" .. playerInQueue].QueueButton.UIStroke.Color = Color3.new(255, 255, 255)
				GridLayout["Frame" .. playerInQueue].QueueButton.UIStroke.Thickness = 2
				
				-- ADD FADE TO BLACK HERE!!! 
				local TweenService = game:GetService("TweenService")
				local tween = TweenService:Create(ScreenGui.Parent.Black.Frame, TweenInfo.new(1), {BackgroundTransparency = 0})
				tween:Play()
				tween.Completed:Connect(function()
					local tween1 = TweenService:Create(ScreenGui.Parent.Black.TeleportText, TweenInfo.new(1), {TextTransparency = 0})
					tween1:Play()
				end)
			end
			)
		end
		frame.Parent = script
	end
	
	
	frame.Parent = GridLayout
	if not frame:FindFirstChild("EventsConnected") then
		handleFrameClicked(frame, index)
		local marker = Instance.new("BoolValue") -- Just a marker to know if events are connected
		marker.Name = "EventsConnected"
		marker.Parent = frame
	end
end

UpdateQueueInfo.OnClientEvent:Connect(function(data)
	-- Create a temporary table to hold the sorted data
	local sortedData = {}
	for index, queueInfo in ipairs(data) do
		table.insert(sortedData, {index=index, queueInfo=queueInfo})
	end

	-- Sort the temporary table by timeLeft in ascending order
	table.sort(sortedData, function(a, b)
		return a.queueInfo[2] < b.queueInfo[2] -- a.queueInfo[2] and b.queueInfo[2] are the timeLeft values
	end)

	-- Create frames based on the sorted order, but use the original index for logic
	for _, sortedEntry in ipairs(sortedData) do
		local originalIndex = sortedEntry.index
		local playerCount, timeLeft, map, gameMode = unpack(sortedEntry.queueInfo)
		createOrUpdateFrame(originalIndex, playerCount, timeLeft, map, gameMode)
	end
end)