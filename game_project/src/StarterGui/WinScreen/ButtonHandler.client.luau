local ExitGameScreen = script.Parent

-- exit confirmation window and the exit game button
local Window = ExitGameScreen:WaitForChild("Window")
--local ExitButton = ExitGameScreen:WaitForChild("ExitButton")

-- prompt buttons to confirm or deny teleport
--local Confirm = Window:WaitForChild("Confirm")
--local Deny = Window:WaitForChild("Deny")
local CloseWindow = Window:WaitForChild("CloseWindow")
local closeWindowButton = CloseWindow:WaitForChild("ImageButton")


closeWindowButton.MouseButton1Click:Connect(function()
	--if game.Players.LocalPlayer.Character.Humanoid.Health == 0 then
	--	print("attempt to respawn")
	--	game.ReplicatedStorage.Remotes.RespawnEvent:FireServer()
	--end
	Window.Visible = false
	script.Parent.ConfettiFrame.Confetti.Enabled = false
end)


game.ReplicatedStorage.Remotes.WinEvent.OnClientEvent:Connect(function(winningNationName)
	script.Parent.ConfettiFrame.Confetti.Enabled = true
	script.Parent.ConfettiFrame.Visible = false
	script.Parent.ConfettiFrame.Sound:Play()
	Window.TextLabel.Text = winningNationName .. " has won!"
	Window.Visible = true
end)