--//
-- Confetti Cannon by Richard, Onogork 2018.
--//
-- Services.
local svcWorkspace = game:GetService("Workspace");
local Camera = svcWorkspace.CurrentCamera;
-- Confetti Defaults.
local shapes = {script:WaitForChild("CircularConfetti"), script:WaitForChild("SquareConfetti")};
local colors = {Color3.fromRGB(168,100,253), Color3.fromRGB(41,205,255), Color3.fromRGB(120,255,68), Color3.fromRGB(255,113,141), Color3.fromRGB(253,255,106)};
-- Module.
local _Confetti = {}; _Confetti.__index = _Confetti;
	-- Set the gravitational pull for the confetti.
	local gravity = Vector2.new(0,1);
	function _Confetti.setGravity(paramVec2)
		gravity = paramVec2;
	end;
	-- Create a confetti particle.	
	function _Confetti.createParticle(paramEmitter, paramForce, paramParent, paramColors)
		local _Particle = {}; setmetatable(_Particle, _Confetti);
		colors = paramColors;
		-- Adjust forces.
		local xforce = paramForce.X; if (xforce < 0) then xforce = xforce * -1; end;
		local distanceFromZero = 0 - xforce;
		paramForce = Vector2.new(paramForce.X, paramForce.Y + (distanceFromZero * 0.75));
		if (paramColors == nil) then paramColors = colors; end; 
		-- Confetti data.		
		_Particle.EmitterPosition = paramEmitter;
		_Particle.EmitterPower = paramForce;
		_Particle.Position = Vector2.new(0,0);
		_Particle.Power = paramForce;
		_Particle.Color = paramColors[math.random(#paramColors)];
		local function getParticle()
			local label = shapes[math.random(#shapes)]:Clone();
			label.ImageColor3 = _Particle.Color;
			label.Parent = paramParent;
			label.Rotation = math.random(360);
			label.Visible = true;
			label.ZIndex = 20;
			return label;
		end;
		_Particle.Label = getParticle();
		_Particle.DefaultSize = 30;
		_Particle.Size = 1; _Particle.Side = -1;
		_Particle.OutOfBounds = false;
		_Particle.Enabled = false;
		_Particle.Cycles = 0;	
		return _Particle;
	end;
	-- Update the position of the confetti.
	function _Confetti:Update(paramDeltaTime)
		if (self.Enabled and self.OutOfBounds) then
			self.Label.ImageColor3 = self.Color;
			self.Position = Vector2.new(0,0);
			self.Power = Vector2.new(self.EmitterPower.X + math.random(10)-5, self.EmitterPower.Y + math.random(10)-5);
			self.Cycles = self.Cycles + 1;
		end;	
		if (
			(not(self.Enabled) and self.OutOfBounds) or
			(not(self.Enabled) and (self.Cycles == 0))) then
				self.Label.Visible = false;
				self.OutOfBounds = true;
				self.Color = colors[math.random(#colors)];
				return;
		else
			self.Label.Visible = true;
		end;
		local startPosition, currentPosition, currentPower = self.EmitterPosition, self.Position, self.Power;
		local imageLabel = self.Label;
		-- Apply change.
		if (imageLabel) then
			-- Update position.
			local newPosition = Vector2.new(currentPosition.X - currentPower.X, currentPosition.Y - currentPower.Y);
			local newPower = Vector2.new((currentPower.X/1.09) - gravity.X, (currentPower.Y/1.1) - gravity.Y);
			local ViewportSize = Camera.ViewportSize;
			imageLabel.Position = UDim2.new(startPosition.X, newPosition.X, startPosition.Y, newPosition.Y);
			self.OutOfBounds = 
				(imageLabel.AbsolutePosition.X > ViewportSize.X and gravity.X > 0) or
				(imageLabel.AbsolutePosition.Y > ViewportSize.Y and gravity.Y > 0) or
				(imageLabel.AbsolutePosition.X < 0  and gravity.X < 0) or
				(imageLabel.AbsolutePosition.Y < 0 and gravity.Y < 0);
			self.Position, self.Power = newPosition, newPower;
			-- Start spinning if it's reached max height.
			if (newPower.Y < 0) then
				if (self.Size <= 0) then
					self.Side = 1;
					imageLabel.ImageColor3 = self.Color;
				end;
				if (self.Size >= self.DefaultSize) then 
					self.Side = -1;
					imageLabel.ImageColor3 = Color3.new(self.Color.r * 0.65, self.Color.g * 0.65, self.Color.b * 0.65);
				end;
				self.Size = self.Size + (self.Side * 2);
				imageLabel.Size = UDim2.new(0, self.DefaultSize, 0, self.Size);				
			end;
		end;
	end;
	-- Stops a confetti from firing again once it's out of bounds.
	function _Confetti:Toggle()
		self.Enabled = not(self.Enabled);
	end;
	
	function _Confetti:SetColors(paramColors)
		colors = paramColors;
	end;
	
return _Confetti;
