# DevForum Announcement Draft: RbxSync v1.2

**Category:** Community Resources
**Tags:** tools, open-source, sync, git, vscode, ai

---

## Forum Post

# RbxSync v1.2 - Zero-Config Sync with AI Integration

RbxSync is a bidirectional sync tool between Roblox Studio and your filesystem. It enables git-based version control, external editor support, and AI-assisted development.

**v1.2** is our biggest release yet, focused on making the tool easier to use while adding powerful new capabilities for AI-assisted development.

## What's New in v1.2

### Zero-Config Mode
Just run `rbxsync serve` in any directory - no configuration file needed. RbxSync automatically detects your project structure:
- Existing `src/` folders
- Rojo project files (`default.project.json`)
- Loose Luau files

```bash
# That's it. No setup required.
rbxsync serve
```

### Luau LSP Integration
After extraction, RbxSync now generates `default.project.json` automatically so Luau LSP works out of the box. Get full intellisense, type checking, and autocompletion for all Roblox APIs in VS Code.

### AI-Powered Development (MCP)
RbxSync is the only sync tool with native AI integration. When connected to Claude Code or other MCP-compatible tools, AI can:
- Extract your game to files
- Edit scripts and sync changes instantly
- Run playtests and see console output in real-time
- Control a character during playtests (move, interact, equip tools)
- Debug errors autonomously

### Performance & Stability
- **Large games supported**: Games with 180k+ instances now extract without freezing
- **Parallel writes**: Extraction is significantly faster
- **Chunked sync**: Large payloads are automatically batched to prevent failures
- **Instance rename tracking**: Renames in Studio sync to renamed files on disk

### Cross-Platform Support
- Windows path handling completely rewritten
- CI testing on macOS, Windows, and Linux
- Consistent behavior across all platforms

## Feature Comparison

| Feature | RbxSync | Rojo | Argon | Pesto |
|---------|---------|------|-------|-------|
| Automatic two-way sync | :white_check_mark: | :gear: Syncback | :white_check_mark: | :gear: Pro ($6.99) |
| One-click game extraction | :white_check_mark: | :gear: Manual setup | :x: | :white_check_mark: |
| Full property serialization | :white_check_mark: JSON | :large_orange_diamond: XML/Binary | :large_orange_diamond: XML | :white_check_mark: |
| Native MCP/AI integration | :white_check_mark: | :x: | :x: | :large_orange_diamond: Sourcemaps |
| E2E testing from CLI | :white_check_mark: | :x: | :x: | :x: |
| Console streaming | :white_check_mark: | :x: | :x: | :x: |
| Zero-config mode | :white_check_mark: | :x: | :x: | :x: |

## Installation

### CLI (Required)
```bash
# macOS
curl -fsSL https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.ps1 | iex
```

### Studio Plugin
[Install from Creator Store](https://create.roblox.com/store/asset/89280418878393/RbxSync)

### VS Code Extension
Search "RbxSync" in the Extensions marketplace.

## Quick Start

1. Install all three components
2. Open your game in Studio
3. Run `rbxsync serve` in terminal
4. Click "Connect" in the RbxSync Studio widget
5. Click "Extract" to pull your game to files

Your game is now version-controlled and editable in VS Code.

## Links

- **GitHub**: https://github.com/devmarissa/rbxsync
- **Documentation**: https://rbxsync.dev/docs
- **Discord**: [link]
- **VS Code Extension**: [Marketplace](https://marketplace.visualstudio.com/items?itemName=rbxsync.rbxsync)

## Feedback

Found a bug? Have a feature request?
- Open an issue on [GitHub](https://github.com/devmarissa/rbxsync/issues)
- Join our Discord for support

---

## Screenshots/GIF Suggestions

### Required Visuals

1. **Hero GIF: Zero-Config Demo** (15-20 seconds)
   - Show empty folder
   - Run `rbxsync serve`
   - Open Studio, connect, extract
   - Edit file in VS Code, see change in Studio
   - Caption: "From zero to synced in under 30 seconds"

2. **Screenshot: Luau LSP Working**
   - VS Code with a script open
   - Autocomplete dropdown showing Roblox API methods
   - Type hints visible
   - Caption: "Full Roblox intellisense in VS Code"

3. **GIF: AI Development Loop** (20-30 seconds)
   - Claude Code terminal
   - AI making code changes
   - Changes appearing in Studio
   - Error in console, AI fixes it
   - Caption: "AI can see errors and fix them in real-time"

4. **Screenshot: Feature Comparison Table**
   - The comparison table from the post, rendered cleanly
   - Use as a quick reference image

5. **Screenshot: VS Code + Studio Side by Side**
   - Split screen showing both apps
   - Same script visible in both
   - Green "Connected" status visible
   - Caption: "Real-time bidirectional sync"

### Optional Visuals

6. **GIF: Large Game Extraction**
   - Terminal showing extraction progress
   - Instance count updating
   - "Extraction complete" message
   - Caption: "Extract games with 100k+ instances"

7. **Screenshot: Project Structure**
   - VS Code file tree showing extracted game
   - ServerScriptService, ReplicatedStorage folders visible
   - Mix of .luau and .rbxjson files

---

## FAQ Section

### General

**Q: How is this different from Rojo?**
A: RbxSync focuses on ease of use and AI integration. Key differences:
- Zero-config mode (no project file required)
- One-click game extraction (vs manual setup)
- Native MCP integration for AI tools
- Console output streaming to terminal
- E2E testing capabilities

**Q: Is it free?**
A: Yes, RbxSync is free and open source.

**Q: Does it work with Team Create?**
A: RbxSync works with local Studio sessions. For Team Create, you'd extract from the Team Create place and use git for collaboration between team members.

**Q: Will it break my existing Rojo project?**
A: RbxSync uses a different file format (.rbxjson vs .model.json) and can coexist with Rojo. However, you should choose one tool per project to avoid conflicts.

### Technical

**Q: What file formats does it use?**
A:
- Scripts: `.luau` files (MyScript.server.luau, MyScript.client.luau, MyScript.luau)
- Instances: `.rbxjson` files with explicit type annotations for all properties
- Assets: Referenced by asset ID, not stored locally

**Q: Does it support all property types?**
A: Yes, RbxSync serializes all property types including Vector3, CFrame, Color3, Enum, Font, etc. See the [property types documentation](https://rbxsync.dev/docs/file-formats/property-types).

**Q: How do I handle terrain?**
A: Terrain is extracted as a binary blob and synced bidirectionally. Enable terrain in the extract command.

**Q: Does it work with packages?**
A: Packages are extracted as their expanded instance tree. Changes sync back but won't update the package itself.

### AI Integration

**Q: What AI tools work with RbxSync?**
A:
- **Claude Code**: Full MCP integration with all tools
- **Cursor**: Works with .cursorrules file for context
- **GitHub Copilot/Codeium**: Standard file editing, no MCP features

**Q: What can the AI actually do?**
A: With MCP tools, AI can:
- Read and write code
- Sync changes to Studio instantly
- Run playtests
- See console output (print, warn, error)
- Control a character during playtests
- Query server-side game state

**Q: Is the AI safe to use?**
A: The AI operates on your local files and Studio instance. It cannot access your Roblox account or publish games. All changes are visible in git.

### Troubleshooting

**Q: "Connection refused" when starting server**
A: Port 44755 may be in use. Run `rbxsync stop` then `rbxsync serve`.

**Q: Changes not appearing in Studio**
A: Check that:
1. Plugin shows "Connected" (green)
2. "Files -> Studio" is enabled in plugin UI
3. File was saved (not just edited)

**Q: Luau LSP not working after extraction**
A: Ensure you have the [Luau LSP extension](https://marketplace.visualstudio.com/items?itemName=JohnnyMorganz.luau-lsp) installed. RbxSync generates `default.project.json` automatically.

---

## Response Templates

### For "Rojo vs RbxSync" questions:

> Both tools serve similar purposes but have different philosophies. Rojo is more established and configurable, while RbxSync prioritizes ease of use and AI integration. Key RbxSync advantages:
> - Zero-config mode (just run `rbxsync serve`)
> - One-click extraction from existing games
> - Native AI/MCP integration for Claude Code
> - Console streaming to terminal
>
> Try RbxSync if you want the simplest setup or are interested in AI-assisted development.

### For "Does it support X property type?" questions:

> RbxSync supports all serializable property types through its `.rbxjson` format. Each property includes explicit type annotations, so there's no ambiguity during deserialization. Check the [property types docs](https://rbxsync.dev/docs/file-formats/property-types) for the full list.

### For "How do I get started?" questions:

> 1. Install the CLI: `curl -fsSL https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.sh | sh`
> 2. Install the [Studio plugin](https://create.roblox.com/store/asset/89280418878393/RbxSync)
> 3. Open your game in Studio
> 4. Run `rbxsync serve` in your project folder
> 5. Connect via the plugin widget, then click Extract
>
> Full guide: https://rbxsync.dev/docs/getting-started

### For bug reports:

> Thanks for the report! Could you provide:
> 1. Your OS (Windows/Mac)
> 2. RbxSync version (`rbxsync version`)
> 3. Steps to reproduce
> 4. Any error messages from terminal or Studio output
>
> You can also open a GitHub issue: https://github.com/devmarissa/rbxsync/issues

### For AI integration questions:

> RbxSync's AI integration works through MCP (Model Context Protocol). Here's how to set it up with Claude Code:
>
> 1. Add to your MCP config:
> ```json
> {
>   "mcpServers": {
>     "rbxsync": {
>       "command": "/path/to/rbxsync-mcp"
>     }
>   }
> }
> ```
> 2. Run `rbxsync serve` and connect Studio
> 3. Claude Code now has access to extraction, sync, testing, and bot tools
>
> Full guide: https://rbxsync.dev/docs/ai-instructions

### For performance concerns:

> v1.2 significantly improves performance for large games:
> - Parallel file writes during extraction
> - Chunked sync requests to prevent payload limits
> - 50ms deduplication to prevent echo loops
>
> Games with 180k+ instances now extract smoothly. If you're still experiencing issues, check that:
> 1. You're on v1.2 (`rbxsync version`)
> 2. Your antivirus isn't scanning the project folder

---

## Post Checklist

- [ ] Update version number in docs before posting
- [ ] Test all installation commands on fresh machine
- [ ] Record hero GIF
- [ ] Take screenshots
- [ ] Verify all links work
- [ ] Cross-post to Twitter/Discord
- [ ] Monitor thread for first 24 hours

---

*Draft prepared: 2026-01-16*
*Version: v1.2*
