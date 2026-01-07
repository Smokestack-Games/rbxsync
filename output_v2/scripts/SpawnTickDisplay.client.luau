-- Reference the ReplicatedStorage
local ReplicatedStorage = game:GetService("ReplicatedStorage")

-- Reference the Data object in ReplicatedStorage
local data = ReplicatedStorage:WaitForChild("Data")

-- Reference the GameStatus value in Data
local gameStatusValue = data:WaitForChild("GameStatus")

local ticksToSpawn = data:WaitForChild("TicksToSpawn")



-- Reference the tick countdown text label (replace "TextLabel" with the actual name of your GUI element)
local countdownLabel = script.Parent:WaitForChild("TextLabel")

local titleLabel = script.Parent:WaitForChild("TitleLabel")

-- Function to update the countdown label text
local function updateCountdownText()
	countdownLabel.Text = ticksToSpawn.Value - 1
end

-- Function to update the countdown based on changes in GameStatus
local function updateCountdown()
	while wait() do
		if gameStatusValue.Value == 1 then
			titleLabel.Visible = true
			updateCountdownText()
		elseif gameStatusValue.Value == 2 then
			countdownLabel:Destroy()
			titleLabel:Destroy()
			break
		end
		--wait(1) -- Update the countdown every second
	end
end

-- Start the countdown when the LocalScript is run
updateCountdown()

-- Connect the LocalScript to changes in ticksToSpawn
data.GameStatus:GetPropertyChangedSignal("Value"):Connect(function()
	if gameStatusValue.Value == 1 then
		updateCountdown()
	end
end)
