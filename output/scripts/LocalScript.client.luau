--!strict
game:GetService("TestService"):Message("This game is using Re:Notify by Sezei / 0bBinary - V1.3A");

local TweenService = game:GetService('TweenService');
local GUI = script.Parent;

local InternalEvent:BindableEvent = GUI:FindFirstChild("InternalCommunication") :: BindableEvent;
local RemoteEvent:RemoteEvent = game:GetService("ReplicatedStorage"):WaitForChild("ReNotifyEvent") :: RemoteEvent;
local RemoteFunction:RemoteFunction = game:GetService("ReplicatedStorage"):WaitForChild("ReNotifyFunction") :: RemoteFunction;

local require = require;
local Cardboard = require(script:FindFirstChild("CardBoard"));

local Listenings = {};
InternalEvent.Event:Connect(function(Action, UID, Option)
	if Action == "NotificationOptionSelected" then
		if Listenings[UID] and type(Listenings[UID]) == "function" then
			Listenings[UID](Option);
		end
		RemoteEvent:FireServer("NotificationOptionSelected", UID, Option);
	end
end);

-- For some reason these values keep resetting, so we have to set them again.
GUI.IgnoreGuiInset = true;
GUI.ResetOnSpawn = false;

local function ClearNotification(Notification:TextButton)
	Notification.Name = "disappearing"
	-- Remove the notification out of the screen
	local currentpos = Notification.Position;
	local targetpos = UDim2.new(currentpos.X.Scale + 2, currentpos.X.Offset, currentpos.Y.Scale, currentpos.Y.Offset);
	local tween = TweenService:Create(Notification, TweenInfo.new(0.5, Enum.EasingStyle.Quad, Enum.EasingDirection.In), {Position = targetpos});
	tween:Play();
	-- Move any other notifications downwards
	for _,v in GUI:GetChildren() do
		if v:IsA("TextButton") then
			if v.Position.Y.Offset < Notification.Position.Y.Offset then
				local currentpos = v.Position;
				local targetpos = UDim2.new(currentpos.X.Scale, currentpos.X.Offset, currentpos.Y.Scale, currentpos.Y.Offset + 40);
				local tween = TweenService:Create(v, TweenInfo.new(0.3, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {Position = targetpos});
				tween:Play();
			end
		end
	end
	-- Delete the notification after the tween is done
	task.wait(0.75);
	Notification:Destroy();
end;

local function PlaySound(AudioString : string) : nil
	local Sound = Instance.new("Sound");
	Sound.SoundId = "rbxassetid://" .. AudioString;
	Sound.Parent = GUI;
	Sound.Looped = false;
	if not Sound.IsLoaded then Sound.Loaded:Wait() end;
	Sound:Play();
	Sound.Ended:Connect(function()
		Sound:Destroy();
	end);

	return nil;
end;

local function CreateNotification(text:string, options:{[any?]: any?}?)
	--[[
	local UID = CreateUID();
	local Notification:TextButton = Assets:FindFirstChild("Notification"):Clone() :: TextButton;
	--]]

	local Notification, UID = Cardboard("Notification", GUI, {
		["Text"] = text;
	});

	local Label:TextLabel = Notification:FindFirstChild("Label") :: TextLabel;

	local Options:Frame = Notification:FindFirstChild("Options") :: Frame;
	local PrimaryOption = nil;
	local TimedOutOption = nil;
	local PushingDown:boolean = false;
	local Clickable:boolean = true;
	local OnePosDown:boolean = false;

	Notification.Name = UID;
	Notification.Parent = GUI;

	if options then
		-- Avoid 'permanent' notifications; They MUST get a way to be removed.
		if typeof(options.Timeout) ~= "number" and typeof(options.Clickable) == "boolean" and options.Clickable == false then
			options.Timeout = 30; -- This makes the notification last for 30 seconds.
			-- For addon devs: If you want a permanent notification, don't make the notification unclickable; It'll clutter the screen.
			-- You can use a hintbox instead for !setmessage, as it will not obscure the notifications box.
		end

		if options.Type and type(options.Type) == "string" then
			if string.lower(options.Type) == "normal" then
				Cardboard:UpdateAsset(UID, {
					["BackgroundColor"] = Color3.fromRGB(45, 45, 45);
					["TextColor"] = Color3.fromRGB(255, 255, 255);
				});
			elseif string.lower(options.Type) == "warning" or string.lower(options.Type) == "warn" then
				Cardboard:UpdateAsset(UID, {
					["BackgroundColor"] = Color3.fromRGB(45, 45, 45);
					["TextColor"] = Color3.fromRGB(255, 127, 0);
				});
			elseif string.lower(options.Type) == "error" then
				Cardboard:UpdateAsset(UID, {
					["BackgroundColor"] = Color3.fromRGB(45, 45, 45);
					["TextColor"] = Color3.fromRGB(255, 60, 60);
				});
			elseif string.lower(options.Type) == "critical" then
				Cardboard:UpdateAsset(UID, {
					["BackgroundColor"] = Color3.fromRGB(255, 60, 60);
					["TextColor"] = Color3.fromRGB(255, 255, 255);
				});
				Label.Font = Enum.Font.GothamBlack;
				PushingDown = true;
			end
		end
		if options.TextColor and typeof(options.TextColor) == "Color3" then
			Cardboard:UpdateAsset(UID, {
				["TextColor"] = options.TextColor;
			});
		end
		if options.BackgroundColor and typeof(options.BackgroundColor) == "Color3" then
			Cardboard:UpdateAsset(UID, {
				["BackgroundColor"] = options.BackgroundColor;
			});
		end
		if options.Timeout and typeof(options.Timeout) == "number" then
			local timer:Frame = Notification:FindFirstChild("Timer") :: Frame;
			timer:TweenSize(UDim2.new(0, 0, 0, 2), Enum.EasingDirection.InOut, Enum.EasingStyle.Linear, options.Timeout, true);
			timer.Visible = true;
			task.spawn(function()
				task.wait(options.Timeout + 0.1);
				if Notification.Parent then
					InternalEvent:Fire("NotificationOptionSelected", UID, TimedOutOption or "Timedout");
					ClearNotification(Notification);
				end
			end)
		end
		if options.PushLowest and type(options.PushLowest) == "boolean" then
			PushingDown = options.PushLowest;
		end
		if type(options.Clickable) == "boolean" then
			Clickable = options.Clickable;
			Notification.AutoButtonColor = options.Clickable;
		end
		if options.Image and type(options.Image) == "string" then
			Cardboard:UpdateAsset(UID, {
				["Icon"] = options.Image;
			});
			Label.Position = UDim2.new(0, 42, 0, 0);
			Notification.Image.Visible = true;
		else
			Label.Position = UDim2.new(0, 10, 0, 0);
			Notification.Image.Visible = false;
		end
		if options.Sound and type(options.Sound) == "string" then
			PlaySound(options.Sound);
		end
		if options.Options and type(options.Options) == "table" then
			local SortedOptions = {};
			local checked = false;
			for internalname, optionvalues in (options.Options) do
				if type(optionvalues) == "string" then
					SortedOptions[internalname] = optionvalues;
					if checked == false then
						PrimaryOption = internalname;
					end
					checked = true;
				elseif type(optionvalues) == "table" then
					SortedOptions[internalname] = internalname;
					if optionvalues.Text and (type(optionvalues.Text) == "string" or type(optionvalues.Text) == "number") then
						SortedOptions[internalname] = tostring(optionvalues.Text);
						if checked == false then
							PrimaryOption = internalname;
						end
					end
					if optionvalues.Primary and type(optionvalues.Primary) == "boolean" and optionvalues.Primary == true then
						PrimaryOption = internalname;
					end
					if optionvalues.TimedOut and type(optionvalues.TimedOut) == "boolean" and optionvalues.TimedOut == true then
						TimedOutOption = internalname;
					end
					checked = true;
				end
			end

			-- Create the primary option
			local Button:TextButton = Options:FindFirstChild("PROMPT_DefaultButton"):Clone() :: TextButton;
			Button.Name = PrimaryOption;
			Button.Text = SortedOptions[PrimaryOption];
			Button.Parent = Options;
			Button.Visible = true;
			Button.Size = UDim2.new(0, Button.TextBounds.X + 20, 0, 30);
			Button.MouseButton1Click:Connect(function()
				InternalEvent:Fire("NotificationOptionSelected", UID, PrimaryOption);
				ClearNotification(Notification);
			end);

			-- Create the other options
			for internalname, text in (SortedOptions) do
				if internalname ~= PrimaryOption then
					local Button:TextButton = Options:FindFirstChild("PROMPT_Button"):Clone() :: TextButton;
					Button.Name = internalname;
					Button.Text = text;
					Button.Parent = Options;
					Button.Visible = true;
					Button.Size = UDim2.new(0, Button.TextBounds.X + 10, 0, 30);
					Button.MouseButton1Click:Connect(function()
						InternalEvent:Fire("NotificationOptionSelected", UID, internalname);
						ClearNotification(Notification);
					end);
				end
			end
		end
	end;

	-- Combine the size of the label and the options
	local ListLayout = Options:FindFirstChild("UIListLayout") :: UIListLayout;
	local XSize = Label.TextBounds.X + ListLayout.AbsoluteContentSize.X + 35;

	if ListLayout.AbsoluteContentSize.X < 2 then
		XSize = Label.TextBounds.X + 20;
	end

	if type(options) == "table" and options.Image and type(options.Image) == "string" then
		XSize = XSize + 32;
	end

	Notification.Size = UDim2.new(0, math.max(XSize, 250), 0, 36);

	-- Find where the notification should go if PushLowest is disabled
	if not PushingDown then
		local NotificationCount = -1;
		for _, notification in (GUI:GetChildren()) do
			if notification:IsA("TextButton") and notification.Name ~= "disappearing" then
				NotificationCount = NotificationCount + 1;
			end
		end
		local NotificationPosition = UDim2.new(1, -10, 1, (-40 * NotificationCount - 82));
		Notification.Position = NotificationPosition;
	else
		local children = GUI:GetChildren();
		for _,v in (children) do
			if v:IsA("TextButton") and v.Name ~= UID and v.Name ~= "disappearing" then
				local currentpos = v.Position;
				local targetpos = UDim2.new(currentpos.X.Scale, currentpos.X.Offset, currentpos.Y.Scale, currentpos.Y.Offset - 40);
				local tween = TweenService:Create(v, TweenInfo.new(0.3, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), {Position = targetpos});
				tween:Play();
			end
		end
	end

	-- On notification click (not on the options)
	Notification.MouseButton1Click:Connect(function()
		if not Clickable then
			return;
		end
		InternalEvent:Fire("NotificationOptionSelected", UID, PrimaryOption or "Clicked");
		Clickable = false;
		ClearNotification(Notification);
	end);

	Notification.Visible = true;

	return Notification, UID;
end;

local function UpdateNotification(Notification:TextButton, Changes:any)
	if type(Notification) == 'string' then
		-- Find the notification
		Notification = GUI:FindFirstChild(Notification) :: TextButton;
	end
	
	if not Notification then return end;
	local Label:TextLabel = Notification:FindFirstChild("Label") :: TextLabel;
	local Options:Frame = Notification:FindFirstChild("Options") :: Frame;
	if type(Changes) == "string" then
		Label.Text = Changes;
	elseif type(Changes) == "table" then
		if Changes.Text and type(Changes.Text) == "string" then
			Label.Text = Changes.Text;
		end
		if Changes.Image and type(Changes.Image) == "string" then
			if Changes.Image ~= "remove" then
				local Image:ImageLabel = Notification:FindFirstChild("Image") :: ImageLabel;
				Image.Image = Changes.Image;
				Image.Visible = true;
				Label.Position = UDim2.new(0, 42, 0, 0);
			else
				local Image:ImageLabel = Notification:FindFirstChild("Image") :: ImageLabel;
				Image.Image = "";
				Image.Visible = false;
				Label.Position = UDim2.new(0, 10, 0, 0);
			end
		end
		if Changes.TextColor and typeof(Changes.TextColor) == "Color3" then
			Label.TextColor3 = Changes.TextColor;
		end
		if Changes.BackgroundColor and typeof(Changes.BackgroundColor) == "Color3" then
			Notification.BackgroundColor3 = Changes.BackgroundColor;
		end
		if Changes.Options and type(Changes.Options) == "table" then
			-- Clear the options
			for _, option in (Options:GetChildren()) do
				if option:IsA("TextButton") and option.Name ~= "PROMPT_DefaultButton" and option.Name ~= "PROMPT_Button" then
					option:Destroy();
				end
			end

			local SortedOptions = {};
			local PrimaryOption;
			local TimedOutOption;
			local checked = false;
			for internalname, optionvalues in (Changes.Options) do
				if type(optionvalues) == "table" then
					if optionvalues.Text and type(optionvalues.Text) == "string" then
						SortedOptions[internalname] = tostring(optionvalues.Text);
						if checked == false then
							PrimaryOption = internalname;
						end
					end
					if optionvalues.Primary and type(optionvalues.Primary) == "boolean" and optionvalues.Primary == true then
						PrimaryOption = internalname;
					end
					if optionvalues.TimedOut and type(optionvalues.TimedOut) == "boolean" and optionvalues.TimedOut == true then
						TimedOutOption = internalname;
					end
					checked = true;
				end
			end

			-- Create the primary option
			local Button:TextButton = Options:FindFirstChild("PROMPT_DefaultButton"):Clone() :: TextButton;
			Button.Name = PrimaryOption;
			Button.Text = SortedOptions[PrimaryOption];
			Button.Parent = Options;
			Button.Visible = true;
			Button.Size = UDim2.new(0, Button.TextBounds.X + 20, 0, 30);
			Button.MouseButton1Click:Connect(function()
				InternalEvent:Fire("NotificationOptionSelected", Notification.Name, PrimaryOption);
				ClearNotification(Notification);
			end);

			-- Create the other options
			for internalname, text in (SortedOptions) do
				if internalname ~= PrimaryOption then
					local Button:TextButton = Options:FindFirstChild("PROMPT_Button"):Clone() :: TextButton;
					Button.Name = internalname;
					Button.Text = text;
					Button.Parent = Options;
					Button.Visible = true;
					Button.Size = UDim2.new(0, Button.TextBounds.X + 10, 0, 30);
					Button.MouseButton1Click:Connect(function()
						InternalEvent:Fire("NotificationOptionSelected", Notification.Name, internalname);
						ClearNotification(Notification);
					end);
				end
			end
		end
	end

	-- Resize the notification
	local ListLayout = Options:FindFirstChild("UIListLayout") :: UIListLayout;
	local XSize = Label.TextBounds.X + ListLayout.AbsoluteContentSize.X + 35;

	if ListLayout.AbsoluteContentSize.X < 2 then
		XSize = Label.TextBounds.X + 20;
	end

	if type(Changes) == "table" and Changes.Image and type(Changes.Image) == "string" then
		XSize = XSize + 32;
	end

	Notification:TweenSize(UDim2.new(0, XSize, 0, 36), Enum.EasingDirection.Out, Enum.EasingStyle.Quad, 0.3, true);
end;

-- Connect the remotes.
RemoteEvent.OnClientEvent:Connect(function(...)
	local Arguments = {...};
	local EventName = Arguments[1];
	table.remove(Arguments, 1);

	if EventName == "Notify" then
		CreateNotification(tostring(Arguments[1]), Arguments[2]);
	elseif EventName == "UpdateNotification" then
		UpdateNotification(Arguments[1], Arguments[2]);
	elseif EventName == "ClearNotification" then
		ClearNotification(Arguments[1]);
	end
end);

RemoteFunction.OnClientInvoke = function(...)
	local Arguments = {...};
	local FunctionName = Arguments[1];
	table.remove(Arguments, 1);

	if FunctionName == "NotificationExists" then
		return (not (not Cardboard:FindAsset(Arguments[1])));
	elseif FunctionName == "Notify" then -- If used in a function, it usually means the server is awaiting for a response from the player due to the .Options; Return an UUID to the server.
		local _, UID = CreateNotification(tostring(Arguments[1]), Arguments[2]);
		return UID;
	end
end;