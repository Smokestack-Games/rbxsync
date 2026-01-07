local TotalTroops = script.Parent

local oldTroopsValue = 16

-- Function to update the tick value and troop count text color
local textLabel = script.Parent.Display.TroopCount

if textLabel then
	textLabel.Changed:Connect(function(property)
		if property == "Text" then
			-- The Text property of the TextLabel has changed
			local newText = textLabel.Text

			-- Remove commas from the newText string
			local newTextNumber = newText:gsub(",", "")


			if tonumber(newTextNumber) < oldTroopsValue then
				TotalTroops.Display.TroopCount.TextColor3 = Color3.new(1, 0, 0)
			else
				TotalTroops.Display.TroopCount.TextColor3 = Color3.new(0, 1, 0)
			end
			-- You can perform any additional actions or checks here
			oldTroopsValue = tonumber(newTextNumber)
		end
	end)
end


