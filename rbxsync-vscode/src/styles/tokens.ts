/**
 * RbxSync Design Tokens
 * Unified design system for visual consistency across all RbxSync interfaces
 */

export const tokens = {
  colors: {
    // Core Palette
    background: '#18181B',      // rgb(24, 24, 27)
    surface: '#202024',         // rgb(32, 32, 36)
    surfaceHover: '#2D2D32',    // rgb(45, 45, 50)
    surfaceActive: '#373740',   // rgb(55, 55, 64)
    border: '#2D2D32',          // rgb(45, 45, 50)
    borderLight: '#3C3C44',     // rgb(60, 60, 68)

    // Accent (Primary Brand Color - Green)
    accent: '#4ADE80',          // rgb(74, 222, 128)
    accentHover: '#5EEA94',     // rgb(94, 234, 148)
    accentMuted: '#22543A',     // rgb(34, 84, 58)
    accentSoft: 'rgba(74, 222, 128, 0.15)',

    // Status Colors
    success: '#4ADE80',         // rgb(74, 222, 128) - same as accent
    warning: '#FACC15',         // rgb(250, 204, 21)
    error: '#F87171',           // rgb(248, 113, 113)
    info: '#60A5FA',            // rgb(96, 165, 250)

    // Soft variants (15% opacity)
    successSoft: 'rgba(74, 222, 128, 0.15)',
    warningSoft: 'rgba(250, 204, 21, 0.15)',
    errorSoft: 'rgba(248, 113, 113, 0.15)',
    infoSoft: 'rgba(96, 165, 250, 0.15)',

    // Text Hierarchy
    textPrimary: '#F4F4F5',     // rgb(244, 244, 245)
    textSecondary: '#A1A1AA',   // rgb(161, 161, 170)
    textMuted: '#71717A',       // rgb(113, 113, 122)
    textAccent: '#4ADE80',      // rgb(74, 222, 128)
  },

  spacing: {
    xs: '4px',
    sm: '8px',
    md: '12px',
    lg: '16px',
    xl: '20px',
    xxl: '24px',
  },

  radius: {
    sm: '4px',
    md: '6px',
    lg: '8px',
    full: '999px',
  },

  typography: {
    // Font sizes
    largeTitle: '16px',
    sectionTitle: '14px',
    cardTitle: '13px',
    body: '12px',
    label: '11px',
    caption: '10px',
    badge: '9px',

    // Font weights
    bold: '700',
    semibold: '600',
    medium: '500',
    regular: '400',
  },

  // Component-specific presets
  components: {
    // Primary Button (Sync)
    primaryButton: {
      background: '#4ADE80',
      backgroundHover: '#5EEA94',
      text: '#18181B',
      border: 'none',
      radius: '6px',
    },

    // Secondary Button (Extract, Test)
    secondaryButton: {
      background: '#202024',
      backgroundHover: '#2D2D32',
      text: '#A1A1AA',
      border: '1px solid #2D2D32',
      radius: '6px',
    },

    // Status Badge
    badge: {
      background: '#4ADE80',
      text: '#FFFFFF',
      fontSize: '9px',
      fontWeight: '600',
      padding: '2px 6px',
      radius: '4px',
    },

    // Status Dot
    statusDot: {
      size: '8px',
      connected: '#4ADE80',
      disconnected: '#71717A',
      connecting: '#FACC15',
    },

    // Toggle Switch
    toggle: {
      trackWidth: '28px',
      trackHeight: '16px',
      knobSize: '12px',
      trackOff: '#71717A',
      trackOn: '#4ADE80',
      knob: '#FFFFFF',
    },
  },
} as const;

/**
 * Generate CSS custom properties from tokens
 */
export function generateCSSVariables(): string {
  return `
    /* Core Palette */
    --bg-base: ${tokens.colors.background};
    --bg-surface: ${tokens.colors.surface};
    --bg-hover: ${tokens.colors.surfaceHover};
    --bg-active: ${tokens.colors.surfaceActive};
    --border: ${tokens.colors.border};
    --border-light: ${tokens.colors.borderLight};

    /* Accent */
    --accent: ${tokens.colors.accent};
    --accent-hover: ${tokens.colors.accentHover};
    --accent-muted: ${tokens.colors.accentMuted};
    --accent-soft: ${tokens.colors.accentSoft};

    /* Status */
    --success: ${tokens.colors.success};
    --success-soft: ${tokens.colors.successSoft};
    --warning: ${tokens.colors.warning};
    --warning-soft: ${tokens.colors.warningSoft};
    --error: ${tokens.colors.error};
    --error-soft: ${tokens.colors.errorSoft};
    --info: ${tokens.colors.info};
    --info-soft: ${tokens.colors.infoSoft};

    /* Text */
    --text-primary: ${tokens.colors.textPrimary};
    --text-secondary: ${tokens.colors.textSecondary};
    --text-muted: ${tokens.colors.textMuted};
    --text-accent: ${tokens.colors.textAccent};

    /* Spacing */
    --space-xs: ${tokens.spacing.xs};
    --space-sm: ${tokens.spacing.sm};
    --space-md: ${tokens.spacing.md};
    --space-lg: ${tokens.spacing.lg};
    --space-xl: ${tokens.spacing.xl};
    --space-xxl: ${tokens.spacing.xxl};

    /* Radius */
    --radius-sm: ${tokens.radius.sm};
    --radius-md: ${tokens.radius.md};
    --radius-lg: ${tokens.radius.lg};
    --radius-full: ${tokens.radius.full};
  `;
}

export default tokens;
