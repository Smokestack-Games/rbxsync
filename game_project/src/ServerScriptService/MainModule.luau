--!strict
-- VERSION V1.3
local RedefineAExists:boolean = false;
if shared.RedefineA then
	RedefineAExists = not not shared.RedefineA.Notify;
end;
local Players = game:GetService("Players");

local RemoteEvent:RemoteEvent = game:GetService("ReplicatedStorage"):FindFirstChild("ReNotifyEvent") or Instance.new("RemoteEvent",game.ReplicatedStorage);
RemoteEvent.Name = 'ReNotifyEvent';

local RemoteFunction:RemoteFunction = game:GetService("ReplicatedStorage"):FindFirstChild("ReNotifyFunction") or Instance.new("RemoteFunction",game.ReplicatedStorage);
RemoteFunction.Name = 'ReNotifyFunction';

export type NotificationChoice = {
	Text:string?|number?;
	Primary:boolean?;
	TimedOut:boolean?;
};

export type Notification = {
	Type:("normal"|"warning"|"warn"|"error"|"critical")?;
	Timeout:number?;
	Clickable:boolean?;
	TextColor:Color3?;
	BackgroundColor:Color3?;
	PushLowest:boolean?;
	Image:string?;
	Sound:string?;
	Options:{NotificationChoice|string}?;
	Callback:((string|number)->any?)?;
};

local activenotifications = {};

local listener;

function AddClient(Player) -- I mean to be fair there's a good chance people would still need to Notify people outside of Redefine:A. That's what this is for.
	if Player.PlayerGui:FindFirstChild("Re:NotifyClient") then return end;

	local Client = script:FindFirstChild("Re:NotifyClient"):Clone();
	Client.Parent = Player.PlayerGui;
	Client.LocalScript.Disabled = false;

	-- some checks
	Client.ResetOnSpawn = false;
	Client.Enabled = true;
end

Players.PlayerAdded:Connect(AddClient)
for _,Player in Players:GetPlayers() do
	AddClient(Player);
end

local function Notify(player:Player, text:string, data:Notification):Notification?
	if RedefineAExists then
		return shared.RedefineA.Notify(player, text, data) :: Notification?;
	end;

	if not data then data = {} end; -- data is completely optional.
	if not listener then
		listener = RemoteEvent.OnServerEvent:Connect(function(player, key, uid, info,...)
			if key == "NotificationOptionSelected" then
				local notification = activenotifications[uid];
				if notification then
					if notification.Callback then
						notification.Callback(info);
					end
					notification.Completed = true;
					notification.SelectedOption = info;
				end
			end
		end)
	end

	-- Check if the provided player is a player instance.
	if player and typeof(player) == "Instance" and player:IsA("Player") then
		-- Check if the player is in the game.
		if player.Parent == game.Players then
			-- Create a notification instance
			local notification = {};

			local UID;
			local success, fail = pcall(function()
				UID = RemoteFunction:InvokeClient(player, "Notify", text, data);
			end)
			if not success then
				warn(fail);
				return;
			end

			activenotifications[UID] = {
				UID = UID; -- repetitive?
				Player = player,
				Text = text,
				Options = data.Options,
				Callback = data.Callback,
				Completed = false,
				SelectedOption = nil,
			};

			notification = activenotifications[UID];

			function notification:AwaitResponse(callback)
				-- Wait for the player to respond to the notification.
				activenotifications[UID].Callback = callback;

				repeat task.wait() until activenotifications[UID].Completed;
				return activenotifications[UID].SelectedOption;
			end

			function notification:Update(options)
				RemoteEvent:FireClient(player, "UpdateNotification", UID, options);
			end

			function notification:Destroy()
				-- Send a signal to the player to destroy the notification on their end.
				RemoteEvent:FireClient(player, "ClearNotification", UID);
				activenotifications[UID] = nil;
			end

			return notification;
		end
	end

	return nil;
end

shared._ReNotify = Notify; -- Support layer for Redefine:A in-case it'll be needed :D

return setmetatable(
	{RemoteEvent = RemoteEvent, RemoteFunction = RemoteFunction},
	{__call = function(self, player:Player, text:string, data:Notification)
		-- we dont need self wtf..
		return Notify(player,text,data);
	end,}
);