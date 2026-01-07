-- Calculate Module
local Calculate = {}


-- adjust for small size (1/12th)
local SMALL_MAP_FACTOR = 1

local CONQUER_COST_PER_PIXEL = 2 --* SMALL_MAP_FACTOR -- Cost in balance to conquer 1 pixel

-- Constants for game rules
local INITIAL_INTEREST = 0.07  -- Initial interest rate

-- remember, actual limit is 0, .01 to 0 is from soft to hard limits
local MIN_INTEREST = 0.01      -- Minimum interest rate
local MAX_INTEREST = 0.026     -- Maximum territorial interest
local INITIAL_TIME = 107-- * 2   -- Initial time in ticks for higher interest rate
local SOFT_LIMIT_MULTIPLIER = 100 -- 100 troops/tile is when it starts to decrease
local HARD_LIMIT_MULTIPLIER = 150 -- hard limit of the density


-- get Troops
local function getTroopValue(player)
	local isBot = typeof(player) ~= "Instance"

	if isBot then
		
		return game.ReplicatedStorage.BotData[tostring(player)].Troops.Value
	else
		return player.leaderstats.Troops.Value
	end
end

-- update Troops
local function updateTroopValue(player, newValue)
	local isBot = typeof(player) ~= "Instance"

	if isBot then
		local oldTroops = game.ReplicatedStorage.BotData[tostring(player)].Troops
		oldTroops.Value = math.max(0, newValue)
	else
		local oldTroops = player.leaderstats.Troops
		oldTroops.Value = math.max(0, newValue)
	end	
end

-- get interest
local function getInterestValue(player)
	local isBot = typeof(player) ~= "Instance"

	if isBot then
		return game.ReplicatedStorage.BotData[tostring(player)].Interest.Value / 100
	else
		return player.leaderstats.Interest.Value / 100
	end
end

-- update interest
local function updateInterestValue(player, newValue)
	local isBot = typeof(player) ~= "Instance"
	
	
	if isBot then
		local oldInterest = game.ReplicatedStorage.BotData[tostring(player)].Interest
		oldInterest.Value = newValue * 100
	else
		--print(newValue)
		local oldInterest = player.leaderstats.Interest
		--print(oldInterest.Value)
		oldInterest.Value = newValue * 100
	end	
end

-- get land
local function getLandValue(player)
	local isBot = typeof(player) ~= "Instance"

	if isBot then
		
		return game.ReplicatedStorage.BotData[tostring(player)].Land.Value
	else
		return player.leaderstats.Land.Value
	end
end

-- update land
local function updateLandValue(playerNation, landChange)
	local userId = playerNation.playerId
	local player = playerNation.isBot and game.ReplicatedStorage.BotData[tostring(userId)] or game.Players:GetPlayerByUserId(userId)
	local landStat = playerNation.isBot and player.Land or player.leaderstats.Land

	-- Adjust the land value safely, ensuring it never goes below zero
	landStat.Value = math.max(0, landStat.Value + landChange)
end





-- Utility functions for interest and limits
local function calculateTerritorialInterest(land)
	-- Ensure territorial interest is between MIN_INTEREST and MAX_INTEREST
	local territorialInterest = MIN_INTEREST + (1.6 * math.sqrt(land)) / 100
	return math.min(territorialInterest, MAX_INTEREST)
end

local function calculateInitialAugmentationFactor(gameTicks)
	-- Ensure the initial augmentation factor decreases from INITIAL_INTEREST to 1 over INITIAL_TIME
	if gameTicks <= INITIAL_TIME then
		local decayFactor = (gameTicks / INITIAL_TIME)
		--print(INITIAL_INTEREST * (1 - decayFactor))
		return math.max(MIN_INTEREST, INITIAL_INTEREST * (1 - decayFactor))
		
	else
		return 1 -- After INITIAL_TIME, the factor is 1 (no augmentation)
	end
end

local function calculateBalanceLimitingFactor(density, softLimit, hardLimit)
	-- Ensure the balance limiting factor is between 0 and 1
	if density < softLimit then
		return 1
	elseif density < hardLimit then
		return (hardLimit - density) / (hardLimit - softLimit)
	else
		return 0
	end
end

-- Function to calculate the current interest rate based on game ticks, troops, and land
local function calculateInterestRate(troops, land, gameTicks)
	local softLimit =  SOFT_LIMIT_MULTIPLIER
	local hardLimit =  HARD_LIMIT_MULTIPLIER
	local density = troops / land

	-- below comment is old code? seems like this is the part that doesnt work as intended
	--local territorialInterest = calculateTerritorialInterest(land)
	
	
	local initialAugmentationFactor = calculateInitialAugmentationFactor(gameTicks)
	local balanceLimitingFactor = calculateBalanceLimitingFactor(density, softLimit, hardLimit)
	
	local finalInterestRate
	
	if initialAugmentationFactor < 1 then
		finalInterestRate = initialAugmentationFactor
	elseif balanceLimitingFactor == 1 then
		-- if below softlimit, between 2.6 (density) and 1 (softlimit)
		-- use inverse density
		-- we use 100 because 100 is the soft limit
		local inverseDensity = SOFT_LIMIT_MULTIPLIER - density
		finalInterestRate = ((1.6 * (inverseDensity/SOFT_LIMIT_MULTIPLIER)) + 1)  / 100
		
	--	finalInterestRate = MAX_INTEREST * balanceLimitingFactor
		
	elseif balanceLimitingFactor > 0 then
		-- if between soft and hard, 1 to 0
		
		-- divide by 10 bc you want 1%, not 10%
		return balanceLimitingFactor /10
	else
		-- should be 0 for 0 interest, ie max density
		return balanceLimitingFactor /10
		
	end
	
	--finalInterestRate = math.max(finalInterestRate, MIN_INTEREST)

	--if density > hardLimit then
	--	finalInterestRate = 0
	--end
	--print (finalInterestRate)
	return finalInterestRate
end

-- Function to update interest rate for a player
function Calculate.updateInterestRate(player, gameTicks)
	local troops = getTroopValue(player)
	local land = getLandValue(player)
	local newInterestRate = calculateInterestRate(troops, land, gameTicks)
	local increment = 0.001
	newInterestRate = math.round(newInterestRate / increment) * increment
	--if newInterestRate == 0 then
		
	--	print(newInterestRate)
	--	print(troops)
	--	print(land)
	--end

	updateInterestValue(player, newInterestRate)
end

-- Function to update player balance due to interest growth
function Calculate.updateBalanceFromInterest(player)
	local interestRate = getInterestValue(player) -- Interest should already be stored as a decimal
	local troops = getTroopValue(player)
	local land = getLandValue(player)
	
	local density = troops/land

	if density >= HARD_LIMIT_MULTIPLIER then
		local newTroops = land * HARD_LIMIT_MULTIPLIER
		updateTroopValue(player, newTroops)
		return
	end
	
	local newTroops = troops * (1 + interestRate)
	
	updateTroopValue(player, newTroops)
end






-- Function to update player balance due to income growth
function Calculate.updateBalanceFromIncome(player)
	
	local troops = getTroopValue(player)
	local land = getLandValue(player)
	
	local density = troops/land
	
	if density >= HARD_LIMIT_MULTIPLIER then
		local newTroops = land * HARD_LIMIT_MULTIPLIER
		updateTroopValue(player, newTroops)
		return
	end
	
	local newTroops = troops + (land * 2) --* 12

	updateTroopValue(player, newTroops)
end

-- Combat mechanics and territory management functions

-- Function to calculate expansion cost
function Calculate.calculateExpansionCost(pixelsToConquer)
	return pixelsToConquer * CONQUER_COST_PER_PIXEL
end


-- Function to conquer new pixels
-- this gets called inside Actions, not ActionHandler
-- Function to conquer new pixels
function Calculate.expandTerritory(player, pixelsToConquer, attackingTroops)
	local costToExpand = Calculate.calculateExpansionCost(pixelsToConquer)
	if attackingTroops >= costToExpand then
		-- send update to client
		--updateTroopValue(player, -costToExpand)
		
		return true, attackingTroops - costToExpand -- Expansion successful, return remaining troops
	else
		local newAttackerTroops = getTroopValue(player)  + attackingTroops
		-- add remainder troops
		updateTroopValue(player, newAttackerTroops)

		return false, attackingTroops -- Not enough balance to expand, return remaining troops
	end
end



-- Function to calculate defending troop density per border pixel
function Calculate.calculateBorderDensity(defenderTroops, defenderLand)
	--  it takes 2 to take out 1
	local density = (defenderTroops / defenderLand) * 2
	return density
end




function Calculate.botAction(botId)
	local botTroops =  getTroopValue(botId)
	local botLand = getLandValue(botId)
	
	local density = (botTroops / botLand)
	if density < math.random(8,32) or botLand < 1 then
		return false
	end
	return true
end




-- Function to calculate attack losses considering border pixels and troop density
function Calculate.calculateAttackLosses(playerDefender, attackingTroops, borderAmount)
	-- Calculate the defending troop density
	local playerTroops = getTroopValue(playerDefender)
	local defendingDensity = Calculate.calculateBorderDensity(playerTroops, getLandValue(playerDefender))

	-- Calculate the number of troops the attacker loses based on the density (assuming a 1:1 ratio)
	--local attackerLosses = 0
	--for i = 1, borderPixels do
	--	if attackingTroops <= 0 then break end

	local attackerLosses = borderAmount * defendingDensity
	
	attackingTroops = attackingTroops - attackerLosses
	
	if attackingTroops < 0 then
		return 0, 0
	end
	--end
	
	
	-- Calculate the number of troops the defender loses (1:0.5 ratio)
	local defenderLosses = math.floor(math.min(attackerLosses/2, playerTroops))
	
	if attackerLosses > playerTroops then
		local newAttackerLosses = math.min(attackerLosses,playerTroops)

		-- this is the leftover troops if you end up wiping out the enemy, these get added back!
		local regainedTroops = attackerLosses - newAttackerLosses
		
		local newTroops = getTroopValue(playerDefender)  + regainedTroops
		updateTroopValue(playerDefender, newTroops)
		return newAttackerLosses, defenderLosses
	end
	
	
	return attackerLosses, defenderLosses
end

-- Function to perform an attack
function Calculate.attack(playerAttacker, playerDefender, attackingTroops, borderAmount)
	-- ... existing logic
	local attackerLosses, defenderLosses = Calculate.calculateAttackLosses(playerDefender, attackingTroops, borderAmount)

	-- Update balances after the attack
	local newAttackerTroops = getTroopValue(playerAttacker)  - attackingTroops
	local newDefenderTroops = getTroopValue(playerDefender)  - defenderLosses
	
	-- update enemy
	updateTroopValue(playerDefender, newDefenderTroops)

	-- Return the results of the attack and the remaining attacking troops vvv
	return attackerLosses, defenderLosses, attackingTroops - attackerLosses
end


return Calculate