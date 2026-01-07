local BordersSlider = require(script.Parent:WaitForChild("BordersSlider"))
local sliderGui = script.Parent:WaitForChild("Slider")

local slider = BordersSlider.create(sliderGui.Background.Bar, sliderGui.Background, sliderGui.Background.Bar.Button, sliderGui.Background.BarBack.BarVisual, sliderGui.Display.TroopCount)