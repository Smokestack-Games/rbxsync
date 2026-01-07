script.Parent:GetPropertyChangedSignal("Text"):Connect(function()
	script.Parent.Size = UDim2.fromOffset(script.Parent.TextBounds.X+15,30)
end)