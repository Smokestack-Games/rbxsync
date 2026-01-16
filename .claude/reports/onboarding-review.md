# Onboarding Review: RbxSync Documentation

**Date:** 2026-01-16
**Reviewer:** New User Perspective Analysis

---

## Executive Summary

RbxSync's onboarding documentation is **functional but has significant room for improvement**. The installation steps are clear, but the overall journey lacks narrative flow, visual aids, and emotional engagement. Users coming from Rojo may find the transition smoother than completely new users.

**Overall Grade:** B-

---

## Step-by-Step Friction Points

### 1. First Encounter (README.md)

| Issue | Severity | Details |
|-------|----------|---------|
| No "What is this?" explanation | Medium | README jumps straight into features comparison. New users need context first: "What problem does this solve?" |
| Feature comparison assumes knowledge | High | Comparing to Rojo/Argon/Pesto assumes user knows these tools. New devs don't. |
| Three-component requirement is intimidating | Medium | Learning you need CLI + Plugin + Extension feels like a lot of setup upfront |

### 2. Getting Started Introduction (docs/getting-started/index.md)

| Issue | Severity | Details |
|-------|----------|---------|
| Too brief | Medium | Only ~25 lines. Doesn't build excitement or explain value proposition deeply |
| Missing "Who is this for?" | Low | No indication if this is for beginners, teams, or advanced users |
| No visual overview | Medium | A diagram showing the sync workflow would help understanding |

### 3. Installation (docs/getting-started/installation.md)

| Issue | Severity | Details |
|-------|----------|---------|
| curl/PowerShell scripts are intimidating | High | Piping scripts from the internet is scary for new devs. No explanation of what they do |
| Manual download instructions incomplete | Medium | Windows PATH instructions say "add to PATH" but don't explain HOW for beginners |
| "Build from source" is unnecessary for most users | Low | Should be even more hidden - takes up screen space |
| No verification step shown prominently | Medium | `rbxsync version` is mentioned but not emphasized as a "you're done!" moment |
| Plugin installation assumes Creator Store familiarity | Low | New Roblox devs may not know what Creator Store is |

### 4. Quick Start (docs/getting-started/quick-start.md)

| Issue | Severity | Details |
|-------|----------|---------|
| "Under 5 minutes" claim sets expectations | Low | Could backfire if it takes longer |
| Project structure shown before user does anything | Medium | User hasn't created project yet; structure is abstract |
| Studio connection steps missing screenshots | High | "Click RbxSync button in toolbar" - where? What does it look like? |
| "Enter your project path" is confusing | High | Full path like `/Users/you/MyGame` vs relative path? Windows paths? |
| Extraction section explains AFTER connection | Good | Logical flow |
| No "Hello World" moment | High | No quick win to verify everything works |

### 5. Configuration (docs/getting-started/configuration.md)

| Issue | Severity | Details |
|-------|----------|---------|
| Too advanced for getting started | Medium | Tree mapping, extraction config, Wally support - overwhelming for new users |
| Should be separate from onboarding | Medium | This is reference material, not getting started |
| Rojo migration is great | Good | Existing users can transition easily |

---

## Missing Information

### Critical Gaps

1. **No "Hello World" tutorial**
   - Users need a complete end-to-end example: create project, make a script, see it in Studio

2. **No screenshots or visual aids**
   - Plugin UI appearance
   - VS Code sidebar location
   - Connection status indicators
   - Error messages and what they mean

3. **No video walkthrough**
   - Complex setup benefits from video demonstration

4. **Prerequisites unclear**
   - Does user need Roblox Studio installed first?
   - Does user need Node.js for VS Code extension?
   - What Studio settings need to be enabled?

5. **No troubleshooting quick-reference in getting started**
   - Common first-time setup errors should be inline

### Missing Explanations

1. **Why files sync the way they do**
   - `.luau` naming conventions are explained but not WHY
   - `.rbxjson` format is described but not motivated

2. **Security concerns not addressed**
   - Why is HttpService required?
   - Is it safe to allow HTTP requests?

3. **Team workflow guidance missing**
   - Getting started assumes solo developer
   - No guidance on setting up for team collaboration

---

## Unclear Instructions

### Ambiguous Steps

1. **"Restart Studio if you just installed the plugin"**
   - How do I know if I "just installed" it? Always restart to be safe?

2. **"Enter your project path"**
   - Absolute or relative? With trailing slash? Forward or backslashes on Windows?

3. **"You should see a green connection indicator"**
   - Where exactly? In the plugin widget? Toolbar? What if it's not green?

4. **"Changes sync to Studio automatically"**
   - When exactly? On save? Immediately? Is there a delay?

5. **"Run `rbxsync sourcemap`"**
   - Why? When? What breaks if I don't?

### Missing Context

1. **`rbxsync serve` runs "on port 44755"**
   - Why does port matter? Do I need to know this? What if blocked?

2. **Binary assets extracted to `.rbxm`**
   - Why binary? Can I edit these? How do I modify a Part?

---

## Suggested Improvements

### High Priority

1. **Add a 5-minute video walkthrough**
   - Record the complete flow from install to first sync
   - Embed at top of getting started page

2. **Create a "Hello World" tutorial**
   ```markdown
   ## Your First Sync (2 minutes)

   1. Create project: `rbxsync init --name HelloWorld`
   2. Add a script: Create `src/ServerScriptService/Hello.server.luau`
   3. Start server: `rbxsync serve`
   4. Open Studio, connect via plugin
   5. See your script appear in ServerScriptService!
   ```

3. **Add screenshots for plugin UI**
   - Show the RbxSync button in Studio toolbar
   - Show the connection widget and each state
   - Show success indicators

4. **Simplify installation page**
   - Lead with ONE command for each platform
   - Hide alternatives in collapsed sections
   - Add big green checkmark verification step

5. **Split Configuration out of Getting Started**
   - Move to separate "Reference" section
   - Getting Started should be minimal

### Medium Priority

6. **Add a "Prerequisites" checklist**
   ```markdown
   Before you begin:
   - [ ] Roblox Studio installed
   - [ ] VS Code (optional)
   - [ ] Terminal/PowerShell access
   ```

7. **Explain the curl/iex scripts**
   - What do they do?
   - Where do they install to?
   - Link to script source

8. **Add inline troubleshooting**
   - "If you see X, try Y" boxes within steps

9. **Add success callouts**
   - After each major step, show what success looks like

10. **Create "Existing Rojo User" fast-track**
    - Separate path for experienced users
    - Skip basic explanations, focus on differences

### Low Priority

11. **Add estimated times to sections**
    - "Installation (~3 min)" etc.

12. **Add a progress indicator**
    - "Step 2 of 4" visual progress

13. **Improve feature comparison table**
    - Add "What does this mean?" tooltips
    - Link to explanations

---

## Comparison to Rojo's Onboarding

### What Rojo Does Better

| Aspect | Rojo | RbxSync |
|--------|------|---------|
| **Motivation first** | Explains WHY before HOW | Jumps to features |
| **Multiple pathways** | "New Game" vs "Existing Game" split | Single linear path |
| **Concept introduction** | Explains sync model conceptually | Assumes understanding |
| **Mature ecosystem** | References Selene, StyLua, Wally integration | Mentions Wally but buried |
| **Progressive disclosure** | Basics first, advanced later | Configuration too early |

### What RbxSync Does Better

| Aspect | RbxSync | Rojo |
|--------|---------|------|
| **One-click install** | curl/PowerShell scripts | Requires manual steps |
| **VS Code integration** | First-party extension | Community solutions |
| **Migration path** | `rbxsync migrate` command | Manual conversion |
| **AI integration docs** | MCP documented upfront | No AI tooling |
| **Extraction workflow** | Clear extract -> sync flow | Syncback is add-on |

### Key Takeaways from Rojo

1. **Rojo explains the "why" before the "how"**
   - RbxSync should add a "Why external files?" section

2. **Rojo offers pathways based on user situation**
   - "Creating a New Game" vs "Porting an Existing Game"
   - RbxSync should add similar branching

3. **Rojo introduces concepts progressively**
   - Project format comes after basic usage
   - RbxSync puts configuration too early

4. **Rojo acknowledges the learning curve**
   - Warns about complexity upfront
   - Sets realistic expectations

---

## Recommended Documentation Restructure

```
Getting Started/
├── Introduction      # What is RbxSync? Why use it?
├── Prerequisites     # NEW: What you need before starting
├── Installation      # Streamlined, one-click focus
├── Hello World       # NEW: First sync in 2 minutes
├── Extraction Guide  # NEW: Getting existing game into files
└── Next Steps        # Where to go from here

Reference/
├── Configuration     # MOVED from Getting Started
├── CLI Commands
├── File Formats
├── Plugin Usage
└── VS Code Extension

Guides/
├── Team Workflow     # NEW: Multi-developer setup
├── Migrating from Rojo
├── AI Development    # MCP integration
└── CI/CD Integration # NEW

Troubleshooting/
├── Common Issues
├── FAQ
└── Getting Help
```

---

## Conclusion

RbxSync has solid technical documentation but lacks the emotional and narrative elements that make onboarding memorable. The tool's value proposition is strong (two-way sync, AI integration, one-click extraction) but gets lost in technical details.

**Priority Actions:**
1. Add screenshots to Quick Start (immediate win)
2. Create "Hello World" tutorial (builds confidence)
3. Record video walkthrough (visual learners)
4. Move Configuration out of Getting Started (reduce overwhelm)
5. Add "Why RbxSync?" motivation section (build excitement)

The documentation is 70% of the way there. With these improvements, the onboarding could go from "adequate" to "delightful."
