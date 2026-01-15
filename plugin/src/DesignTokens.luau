--!strict
-- RbxSync Design Tokens
-- Unified design system for visual consistency across all RbxSync interfaces

local DesignTokens = {}

-- ============================================================================
-- COLORS
-- ============================================================================

DesignTokens.Colors = {
	-- Core Palette
	background = Color3.fromRGB(24, 24, 27),      -- #18181B
	surface = Color3.fromRGB(32, 32, 36),         -- #202024
	surfaceHover = Color3.fromRGB(45, 45, 50),    -- #2D2D32
	surfaceActive = Color3.fromRGB(55, 55, 64),   -- #373740
	border = Color3.fromRGB(45, 45, 50),          -- #2D2D32
	borderLight = Color3.fromRGB(60, 60, 68),     -- #3C3C44

	-- Accent (Primary Brand Color - Green)
	accent = Color3.fromRGB(74, 222, 128),        -- #4ADE80
	accentHover = Color3.fromRGB(94, 234, 148),   -- #5EEA94
	accentMuted = Color3.fromRGB(34, 84, 58),     -- #22543A (for button backgrounds)
	-- accentSoft = 15% opacity of accent (handle in code)

	-- Status Colors
	success = Color3.fromRGB(74, 222, 128),       -- #4ADE80 (same as accent)
	warning = Color3.fromRGB(250, 204, 21),       -- #FACC15
	error = Color3.fromRGB(248, 113, 113),        -- #F87171
	info = Color3.fromRGB(96, 165, 250),          -- #60A5FA

	-- Text Hierarchy
	textPrimary = Color3.fromRGB(244, 244, 245),  -- #F4F4F5
	textSecondary = Color3.fromRGB(161, 161, 170), -- #A1A1AA
	textMuted = Color3.fromRGB(113, 113, 122),    -- #71717A
	textAccent = Color3.fromRGB(74, 222, 128),    -- #4ADE80
}

-- ============================================================================
-- SPACING (4px Grid)
-- ============================================================================

DesignTokens.Spacing = {
	xs = 4,   -- tight
	sm = 8,   -- default gap
	md = 12,  -- section padding
	lg = 16,  -- card padding
	xl = 20,  -- large spacing
	xxl = 24, -- section margins
}

-- ============================================================================
-- BORDER RADIUS
-- ============================================================================

DesignTokens.Radius = {
	sm = 4,    -- buttons, badges
	md = 6,    -- cards, inputs
	lg = 8,    -- dialogs, panels
	full = 999, -- pills, toggles
}

-- ============================================================================
-- TYPOGRAPHY
-- ============================================================================

DesignTokens.Typography = {
	-- Font sizes
	largeTitle = 16,
	sectionTitle = 14,
	cardTitle = 13,
	body = 12,
	label = 11,
	caption = 10,
	badge = 9,

	-- Font weights (Roblox Gotham family)
	fontBold = Enum.Font.GothamBold,
	fontSemibold = Enum.Font.GothamMedium,
	fontMedium = Enum.Font.GothamMedium,
	fontRegular = Enum.Font.Gotham,
}

-- ============================================================================
-- COMPONENT PRESETS
-- ============================================================================

DesignTokens.Components = {
	-- Primary Button (Sync)
	primaryButton = {
		background = DesignTokens.Colors.accent,
		backgroundHover = DesignTokens.Colors.accentHover,
		text = DesignTokens.Colors.background,
		radius = 6,
		height = 40,
		textSize = 12,
		font = Enum.Font.GothamBold,
	},

	-- Secondary Button (Extract, Test)
	secondaryButton = {
		background = DesignTokens.Colors.surface,
		backgroundHover = DesignTokens.Colors.surfaceHover,
		text = DesignTokens.Colors.textSecondary,
		border = DesignTokens.Colors.border,
		radius = 6,
		height = 40,
		textSize = 12,
		font = Enum.Font.GothamBold,
	},

	-- Ghost Button (Dismiss, Cancel)
	ghostButton = {
		background = Color3.fromRGB(0, 0, 0), -- transparent
		backgroundTransparency = 1,
		backgroundHover = DesignTokens.Colors.surfaceHover,
		text = DesignTokens.Colors.textMuted,
		radius = 6,
	},

	-- Card
	card = {
		background = DesignTokens.Colors.surface,
		border = DesignTokens.Colors.border,
		radius = 8,
		padding = 12,
	},

	-- Card (Linked State)
	cardLinked = {
		background = Color3.fromRGB(28, 36, 30), -- subtle green tint
		border = DesignTokens.Colors.accent,
		accentBar = DesignTokens.Colors.accent,
		accentBarWidth = 4,
	},

	-- Status Badge
	badge = {
		background = DesignTokens.Colors.accent,
		text = Color3.fromRGB(255, 255, 255),
		textSize = 9,
		font = Enum.Font.GothamBold,
		paddingH = 6,
		paddingV = 2,
		radius = 4,
	},

	-- Status Dot
	statusDot = {
		size = 8,
		connected = DesignTokens.Colors.accent,
		disconnected = DesignTokens.Colors.textMuted,
		connecting = DesignTokens.Colors.warning,
		glowSize = 16,
		glowTransparency = 0.7,
	},

	-- Toggle Switch
	toggle = {
		trackWidth = 28,
		trackHeight = 16,
		knobSize = 12,
		trackOff = DesignTokens.Colors.textMuted,
		trackOn = DesignTokens.Colors.accent,
		knob = Color3.fromRGB(255, 255, 255),
	},
}

return DesignTokens
