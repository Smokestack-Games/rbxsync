local monthStrMap = {
	Jan=1,
	Feb=2,
	Mar=3,
	Apr=4,
	May=5,
	Jun=6,
	Jul=7,
	Aug=8,
	Sep=9,
	Oct=10,
	Nov=11,
	Dec=12
}
local HttpService = game:GetService("HttpService")
local function RFC2616DateStringToUnixTimestamp(dateStr)
	local day, monthStr, year, hour, min, sec = dateStr:match(".*, (.*) (.*) (.*) (.*):(.*):(.*) .*")
	local month = monthStrMap[monthStr]
	local date = {
		day=day,
		month=month,
		year=year,
		hour=hour,
		min=min,
		sec=sec
	}

	return os.time(date)
end

local isInited = false
local originTime = nil
local responseTime = nil
local responseDelay = nil
local function inited()
	return isInited
end

local function init()
	if not isInited then
		local ok = pcall(function()
			local requestTime = tick()
			local response = HttpService:RequestAsync({Url="http://google.com"}) 
			local dateStr = response.Headers.date
			originTime = RFC2616DateStringToUnixTimestamp(dateStr)
			responseTime = tick()
			-- Estimate the response delay due to latency to be half the rtt time
			responseDelay = (responseTime-requestTime)/2
		end)
		if not ok then
			warn("Cannot get time from google.com. Make sure that http requests are enabled!")
			originTime = os.time()
			responseTime = tick()
			responseDelay = 0
		end

		isInited = true
	end
end

local function time()
	if not isInited then
		init()
	end

	return originTime + tick()-responseTime - responseDelay
end

return {
	inited=inited,
	init=init,
	time=time
}