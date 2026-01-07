--//
-- Confetti Cannon by Richard, Onogork 2018.
--//
local svcRun = game:GetService("RunService");
local ConfettiCannon = require(script.ConfettiParticles);
ConfettiCannon.setGravity(Vector2.new(0,1));
local confetti = {};
-- Create confetti paper.
local AmountOfConfetti = 25;
for i=1, AmountOfConfetti do
	local p = ConfettiCannon.createParticle(
		Vector2.new(0.5,1), 									-- Position on screen. (Scales)
		Vector2.new(math.random(90)-45, math.random(70,100)), 		-- The direction power of the blast.
		script.Parent, 												-- The frame that these should be displayed on.
		{Color3.fromRGB(255,255,100), Color3.fromRGB(255,100,100)} 	-- The colors that should be used.
	);
	table.insert(confetti, p);
end;

local confettiColors = {Color3.fromRGB(255,255,100), Color3.fromRGB(255,100,100)};
local confettiActive = false;
-- Update position of all confetti.
svcRun.RenderStepped:Connect(function()
	for _,val in pairs(confetti) do
		if (confettiColors) then val:SetColors(confettiColors); end;
		val.Enabled = confettiActive;
		val:Update();
	end;
end);

local fire = function(paramColors)
	confettiColors = paramColors;
	spawn(function()
		confettiActive = true;
		wait(tick);
		confettiActive = false;
	end);
end;
while wait(1) do
	fire({Color3.fromRGB(255,0,0), Color3.fromRGB(0,255,0), Color3.fromRGB(0,0,255)});
end;