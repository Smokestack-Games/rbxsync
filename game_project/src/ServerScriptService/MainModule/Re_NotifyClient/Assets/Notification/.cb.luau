--[[
    {
        [Property] = {Table of expected instances, PropertyType, DefaultValue, ExpectedType?};
        
    }
]]

return {
	["Text"] = {
		{
			script.Parent.Label;
		};
		"Text";
		"Notification";
		"string";
	};
	["BackgroundColor"] = {
		{
			script.Parent;
		};
		"BackgroundColor3";
		Color3.fromRGB(45, 45, 45);
		"color3";
	};
	["TextColor"] = {
		{
			script.Parent.Label;
		};
		"TextColor3";
		Color3.fromRGB(255, 255, 255);
		"color3";
	};
	["Icon"] = {
		{
			script.Parent.Image;
		};
		"Image";
		"";
		"string";
	};
}