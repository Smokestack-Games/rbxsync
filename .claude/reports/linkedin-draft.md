# LinkedIn Post Draft: RbxSync v1.2 Beta Announcement

**Issue:** RBXSYNC-9
**Date:** 2026-01-16
**Target Audience:** Roblox developers, game studios, technical recruiters

---

## Main Post (2,847 characters)

**Announcing RbxSync v1.2 Beta: The First AI-Native Sync Tool for Roblox**

After months of development and feedback from the Roblox community, I'm excited to announce RbxSync v1.2 is now in beta.

**The Problem We Solved**

If you've worked on a Roblox game with a team, you know the pain: "Which version is the latest?" "Can you send me that script?" "I accidentally overwrote your changes."

Existing tools helped, but they all shared one major limitation: they were built for humans only. As AI becomes central to game development, we need tools designed for both.

**What Makes RbxSync Different**

RbxSync is the first Roblox sync tool with native AI integration via Model Context Protocol (MCP). This means AI coding assistants like Claude Code can:

- Read and write game files directly
- Execute Luau code in Studio
- Run playtests and see console output in real-time
- Control test characters via pathfinding
- Debug issues autonomously

This isn't a plugin or workaround. It's built into the architecture.

**Key Features in v1.2**

1. **Full Property Serialization** - Every property type (70+) preserved via our .rbxjson format. No more lost attributes or broken references.

2. **True Two-Way Sync** - Edit in Studio or VS Code. Changes propagate instantly. No manual sync buttons.

3. **One-Click Extraction** - Extract any existing game to a Git repo in seconds. No project file configuration needed.

4. **E2E Testing from CLI** - Run playtests, capture console output, automate QA. Perfect for CI/CD pipelines.

5. **Performance** - Large game extraction in under 5 seconds. Small file writes under 50ms.

**Who Is This For?**

- Solo developers wanting version control and AI assistance
- Studios needing reliable collaboration and code review workflows
- Teams building CI/CD pipelines for Roblox
- Anyone tired of "which version?" conversations

**Get Started**

The beta is open now. Install takes under 2 minutes:

```
curl -fsSL https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.sh | sh
```

Documentation: [link]
Discord: [link]
GitHub: [link]

I'd love your feedback. What features matter most? What's missing? Drop a comment or join our Discord.

---

## Alternative Version 1: Short (1,394 characters)

**RbxSync v1.2 Beta is live: AI meets Roblox development**

We built RbxSync because existing sync tools weren't designed for the AI era.

What's new in v1.2:

- Native AI integration (MCP) - AI assistants can read/write game files, run playtests, and debug autonomously
- Full property sync - All 70+ property types preserved
- True two-way sync - Edit in Studio or VS Code, changes propagate instantly
- One-click extraction - Any game to Git in seconds
- E2E testing - CLI-based playtests with console capture

This is for Roblox developers who want:
- Real version control
- AI-assisted coding
- Reliable team collaboration
- Automated testing pipelines

Beta is open. Install in 2 minutes:
```
curl -fsSL https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.sh | sh
```

Feedback welcome. What features would help your workflow?

---

## Alternative Version 2: Longer/Technical (3,842 characters)

**Why I Built an AI-Native Sync Tool for Roblox (and What v1.2 Brings)**

Two years ago, I was leading a Roblox game project with four developers. Every week, we'd lose hours to the same problems:

- Merge conflicts from copy-pasting scripts
- "What's the latest version?" conversations
- Lost work from accidental overwrites
- No way to do proper code review

Sound familiar?

I evaluated every sync tool available. Rojo was powerful but required manual setup and didn't capture all properties. Argon had limitations. Pesto's full features were paid.

More importantly: none of them were designed for how I actually wanted to work. I use AI assistants daily. They're my pair programmers. But they couldn't touch my Roblox games.

So I built RbxSync.

**The Architecture Decision That Changed Everything**

RbxSync isn't just "another sync tool." It's built around a core belief: AI is becoming central to game development, and our tools need to support that natively.

The key innovation is MCP (Model Context Protocol) integration. This is the same protocol used by Claude Code and other AI tools. With RbxSync, an AI assistant can:

1. **Read game state** - Query instances, properties, scripts
2. **Write changes** - Modify files that auto-sync to Studio
3. **Execute code** - Run Luau in Studio context
4. **Test gameplay** - Start playtests, capture console output
5. **Control characters** - Move, jump, equip, interact via pathfinding

This enables workflows that weren't possible before. "Add a leaderboard that tracks player kills" becomes a conversation, not a day of work.

**What's New in v1.2**

Beyond AI integration, v1.2 brings major improvements:

**Property Serialization**
We developed .rbxjson, a JSON format with explicit type annotations. It captures all 70+ Roblox property types: Vector3, CFrame, Color3, Fonts, you name it. No more lossy XML conversions.

**Performance**
- Large game extraction: < 5 seconds
- File writes: < 50ms for 100 files
- JSON serialization: < 1ms for typical payloads

**Developer Experience**
- Zero-config mode for quick starts
- VS Code extension with one-click connect
- Sourcemap generation for Luau LSP
- Build to .rbxl/.rbxm with --watch mode

**Reliability**
- Echo prevention for sync loops
- 50ms deduplication window
- Stable instance IDs across sessions

**The Roadmap**

v1.2 is a beta. Here's what's coming:

- Asset management (images, sounds, meshes)
- Team collaboration features
- Cloud sync options
- More AI tool integrations

**Try It**

Installation takes 2 minutes. The beta is free and open.

I'm actively looking for feedback from:
- Solo developers testing AI workflows
- Studios evaluating collaboration tools
- Anyone building CI/CD for Roblox

Links in comments.

---

## Alternative Version 3: Casual (1,847 characters)

**We made a thing and it's pretty cool**

So... we've been quietly building RbxSync for a while now. It's a sync tool for Roblox that actually works the way you'd expect.

But the reason I'm posting: v1.2 has AI integration built in.

What does that mean? If you use Claude Code (or similar), it can now:
- Read/write your game files
- Run code in Studio
- Start playtests and see the output
- Even control a test character

Basically, you can say "add a door that needs a key to open" and watch it happen. It's wild.

Other stuff in v1.2:
- Full property sync (finally, ALL the properties)
- Real two-way sync (edit anywhere, changes appear everywhere)
- Extract any game to Git with one command
- Actual performance (large games in seconds)

It's free. It's open source. We just want people to try it.

Install: [one-liner in comments]
Docs: [link]
Discord: [link]

Would love to hear what works, what doesn't, what features you need.

---

## Suggested Hashtags

**Primary (use 3-5):**
- #RobloxDev
- #GameDev
- #AI
- #OpenSource
- #Roblox

**Secondary (use 1-2 if space allows):**
- #DevTools
- #GameDevelopment
- #IndieDev
- #Claude
- #LuauLang

**Recommended combination:**
`#RobloxDev #GameDev #AI #OpenSource`

---

## Best Posting Times

**Optimal days:** Tuesday, Wednesday, Thursday

**Optimal times (PST):**
- 8:00-9:00 AM (morning professionals)
- 12:00-1:00 PM (lunch browsers)
- 5:00-6:00 PM (end of workday)

**Best single slot:** Tuesday or Wednesday, 8:30 AM PST

**Reasoning:**
- Roblox developers skew younger (students active evenings/weekends) but professionals check LinkedIn during work hours
- Game studios operate standard business hours
- Technical recruiters most active mid-week
- Avoid Monday (inbox clearing) and Friday (weekend prep)

---

## Follow-up Comment Suggestions

**Comment 1 (Installation):**
```
Quick install:

macOS/Linux:
curl -fsSL https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.sh | sh

Windows (PowerShell):
irm https://raw.githubusercontent.com/devmarissa/rbxsync/master/scripts/install.ps1 | iex

Full docs: [link]
```

**Comment 2 (Engagement prompt):**
```
Curious what sync tools you're currently using for Roblox development? Rojo? Argon? Something else? What's working and what isn't?
```

**Comment 3 (Technical detail):**
```
For the technically curious: RbxSync uses a Rust backend for performance, communicates with Studio via HTTP on localhost, and implements MCP (Model Context Protocol) for AI integration. Happy to answer architecture questions.
```

**Comment 4 (Community):**
```
Join our Discord for support, feature requests, and early access to new releases: [link]
```

---

## Notes for Posting

1. **Include visual content** - Screenshots of VS Code + Studio side-by-side, or a short video showing AI integration in action would significantly boost engagement

2. **Tag relevant accounts** - Consider tagging @Roblox, @anthropic (Claude), relevant Roblox developer influencers

3. **Pin the installation comment** - Makes it easy for interested developers to get started

4. **Respond to comments quickly** - First 1-2 hours are critical for algorithm visibility

5. **Cross-post timing** - If posting to Twitter/X, space them 30-60 minutes apart with slightly different copy
