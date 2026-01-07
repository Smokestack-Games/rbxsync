local ExitGameScreen = script.Parent

-- exit confirmation window and the exit game button
local Window = ExitGameScreen:WaitForChild("Window")
local ExitButton = ExitGameScreen:WaitForChild("ExitButton")

-- prompt buttons to confirm or deny teleport
local Confirm = Window:WaitForChild("Confirm")
local Deny = Window:WaitForChild("Deny")
local CloseWindow = Window:WaitForChild("CloseWindow")
local closeWindowButton = CloseWindow:WaitForChild("ImageButton")


-- complete logic that displays Window when ExitButton is pressed and if confirm is pressed the player teleports, and if deny is pressed the window closes
ExitButton.MouseButton1Click:Connect(function()
	if Window.Visible == true then
		Window.Visible = false
	else
		Window.Visible = true
	end
end)

Confirm.MouseButton1Click:Connect(function()
	-- teleport player to main place
	task.spawn(function()

		-- ADD FADE TO BLACK HERE!!! 
		local TweenService = game:GetService("TweenService")
		local tween = TweenService:Create(script.Parent.Parent.Black.Frame, TweenInfo.new(1), {BackgroundTransparency = 0})
		tween:Play()
		tween.Completed:Connect(function()
			local tween1 = TweenService:Create(script.Parent.Parent.Black.TeleportText, TweenInfo.new(1), {TextTransparency = 0})
			tween1:Play()
		end)
	end
	)
	game:GetService("TeleportService"):Teleport(15149402333, game.Players.LocalPlayer)
	-- fade the UI
	-- send remote to server ?
end)

Deny.MouseButton1Click:Connect(function()
	if game.Players.LocalPlayer.Character.Humanoid.Health == 0 then
		print("attempt to respawn")
		game.ReplicatedStorage.Remotes.RespawnEvent:FireServer()
	end
	Window.Visible = false
end)

closeWindowButton.MouseButton1Click:Connect(function()
	if game.Players.LocalPlayer.Character.Humanoid.Health == 0 then
		print("attempt to respawn")
		game.ReplicatedStorage.Remotes.RespawnEvent:FireServer()
	end
	Window.Visible = false
end)