-- this script needs to update in-game billboard UIs
-- recieve remotes from server to display Troop losses, donations, and also prompt client for ally requests.

-- it should also handle the modification of Nation UI Tags to display Ally colored names for the client nations' allies.

--local Player = game.Players.LocalPlayer

--local ReNotify = require(15700220413);

--local Notification = ReNotify( Player , "Text" , {Options} ); 
---- You can see what the Options are in the Cheat Sheet.

---- V1.2: Send the same notification to all players with a RemoteEvent. No server-sided control, however.
--ReNotify.RemoteEvent:FireAllClients("Text" , {Options} );

-- ally request reciever

-- recieve remote from server when ally request is recieved from another player
-- prompt the player with the notification
-- send remote accepting or denying
-- on server, spawn task that ensures accepting allies is verified by server so hackers dont hack


-- Allies Tags handler

-- recieve remote from server when allyship occurs, and when it breaks (when you attack an ally or send a boat to them)

local AllyRemote = game.Players.LocalPlayer:WaitForChild("RemoteEvent")

AllyRemote.OnClientEvent:Connect(function(targetPlayer, isAlly)
	local tag = game.Workspace.Tags:FindFirstChild(tostring(targetPlayer))
	if isAlly then
		tag.PlayerName.TextColor3 = Color3.new(1, 1, 0)
	else
		tag.PlayerName.TextColor3 = Color3.new(1, 1, 1)
	end
	
end)

-- when receiving remote, determine if its adding or removing (isAlly boolean)
-- find the nation Tag in workspace.Tags(playerId)
-- change the color of (tag).PlayerName.TextColor3 (255,255,255 is default white, swap between yellow (253, 248, 0) and white. no third color needed


