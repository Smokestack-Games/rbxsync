script.Parent:GetPropertyChangedSignal("Text"):Connect(function()
	script.Parent.Size = UDim2.fromOffset(math.max(script.Parent.TextBounds.X+15,75),30)
end)