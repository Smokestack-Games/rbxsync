
local Players = game:GetService("Players")
local ReplicatedFirst = game:GetService("ReplicatedFirst")
local StarterGui = game:GetService("StarterGui")


local player = Players.LocalPlayer
local playerGui = player:WaitForChild("PlayerGui")

local screenGui = script.Parent
screenGui.IgnoreGuiInset = true
screenGui.Parent = playerGui

ReplicatedFirst:RemoveDefaultLoadingScreen()

StarterGui:SetCoreGuiEnabled(Enum.CoreGuiType.Backpack, false)
StarterGui:SetCoreGuiEnabled(Enum.CoreGuiType.Health, false)

-- Function to disable/enable core GUI elements
local function toggleCoreGui(enable)
	StarterGui:SetCoreGuiEnabled(Enum.CoreGuiType.PlayerList, enable)

end

-- Initially disable the core UI
toggleCoreGui(false)

local quoteLabel = script.Parent.Quote
local authorLabel = script.Parent.Author

local quoteTable = {
	{
		quote = "We are defined not by our borders but by our bonds.",
		author = "Barack Obama",
	},
	{
		quote = "A nation that cannot control its borders is not a nation.",
		author = "Ronald Reagan",
	},
	{
		quote = "When goods do not cross borders, soldiers will.",
		author = "Frederic Bastiat",
	},
	{
		quote = "War is peace. Freedom is slavery. Ignorance is strength.",
		author = "George Orwell, 1984",
	},
	{
		quote = "God created war so that Americans would learn geography.",
		author = "Mark Twain",
	},
	{
		quote = "Only the dead have seen the end of war.",
		author = "Plato",
	},
	{
		quote = "War does not determine who is right — only who is left.",
		author = "Anonymous",
	},
	{
		quote = "Real wars are not games, and not fun.",
		author = "Wingman8",
	},
	{
		quote = "It is my conviction that killing under the cloak of war is nothing but an act of murder.",
		author = "Albert Einstein",
	},
	{
		quote = "I'm fed up to the ears with old men dreaming up wars for young men to die in.",
		author = "George McGovern",
	},
	{
		quote = "A country without borders is not a country.",
		author = "Carlos Beruff",
	},
	{
		quote = "Whoever controls the territory possesses it.",
		author = "Paul Virilio",
	},
	{
		quote = "Enemy-occupied territory - that is what this world is.",
		author = "C. S. Lewis",
	},
	{
		quote = "The opportunity of defeating the enemy is provided by the enemy himself.",
		author = "Sun Tzu",
	},
	{
		quote = "Where am I?",
		author = "Joe Biden",
		chance = 0.001, -- Adding a special field to denote the rare chance
	},
	{
		quote = "Do you know, my son, with what little understanding the world is ruled?",
		author = "Anonymous",
	},
	{
		quote = "My first wish is to see this plague of mankind, war, banished from the earth.",
		author = "George Washington, 1st US President",
	},
	{
		quote = "If a man does not know to what port he is sailing, no wind is favorable.",
		author = "Anonymous",
	},
	{
		quote = "An alliance with the strong is never to be trusted.",
		author = "Anonymous",
	}
}

local function GetRandomQuote()
	local randomIndex = math.random(1, #quoteTable)
	local chosenQuote = quoteTable[randomIndex]

	if chosenQuote.author == "Joe Biden" then
		if math.random() > chosenQuote.chance then
			return GetRandomQuote() -- If the random chance doesn't hit, get another quote
		end
	end

	quoteLabel.Text = chosenQuote.quote
	authorLabel.Text = "- " .. chosenQuote.author
end

GetRandomQuote()

task.wait()

local TweenService = game:GetService("TweenService")
local tween1 = TweenService:Create(quoteLabel, TweenInfo.new(1), {TextTransparency = 0})
local tween2 = TweenService:Create(authorLabel, TweenInfo.new(1), {TextTransparency = 0})
tween1:Play()
tween2:Play()

-- play this audio (1839866126) on the player as an intro sound
local introSound = Instance.new("Sound")
introSound.SoundId = "rbxassetid://1839866126"
introSound.Parent = game.Workspace
introSound.Volume = 0.2

local ReplicatedStorage = game:GetService("ReplicatedStorage")


local total = 250*250
local partsLoaded = script.Parent.Background.PartsLoaded
local barVisual = script.Parent.Background.BarVisual
local barBack = script.Parent.Background.BarBack
local gate = true
while gate do
	if ReplicatedStorage:WaitForChild("Data").GameStatus.Value == 1 then
		gate = false
	end
	--if ReplicatedStorage:FindFirstChild("Data").LoadProgress.Value then
		
		partsLoaded.Text = tostring(ReplicatedStorage.Data.LoadProgress.Value) .. "/" .. tostring(total)
		barVisual.Size = UDim2.new(ReplicatedStorage.Data.LoadProgress.Value/total,0,0,10)
	--end
	task.wait(.1)
end

--task.wait()
task.spawn(function()
	task.wait(.5)
	introSound:Play()
	introSound.Ended:Connect(function()
		introSound:Destroy()
	end)
end)

partsLoaded.Text = tostring(total) .. "/" .. tostring(total)
barVisual.Size = UDim2.new(1,0,0,10)
toggleCoreGui(true)

local tween1 = TweenService:Create(quoteLabel, TweenInfo.new(1), {TextTransparency = 1})
local tween2 = TweenService:Create(authorLabel, TweenInfo.new(1), {TextTransparency = 1})
local tween3 = TweenService:Create(screenGui.Black, TweenInfo.new(1), {BackgroundTransparency = 1})

local tween4 = TweenService:Create(screenGui.Background.PartsLoaded, TweenInfo.new(1), {TextTransparency = 1})
local tween5 = TweenService:Create(screenGui.Background.BarVisual, TweenInfo.new(1), {BackgroundTransparency = 1})
local tween6 = TweenService:Create(screenGui.Background.BarBack, TweenInfo.new(1), {BackgroundTransparency = 1})

tween1:Play()
tween2:Play()
tween3:Play()

tween4:Play()
tween5:Play()
tween6:Play()


-- fixing slidr?
tween6.Completed:Connect(function()
	screenGui:Destroy()
end)