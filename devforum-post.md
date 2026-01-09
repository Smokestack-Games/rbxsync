# RbxSync - The sync tool that should have been.

![rbxsynclogo|499x490, 50%](upload://d4Id28Kia6oV6UmYC8Lsq68HNiG.png)

<div align="center">

**One-click extraction, two-way sync for Scripts, Parts, GUIs, Animations & more.**
**Native AI integration & E2E testing. Zero setup.**

</div>

---

Hey everyone,

I'm releasing **RbxSync** as a free beta - a professional-grade sync tool that does two-way sync, full property preservation, and AI integration out of the box.

---

# What is RbxSync?

RbxSync syncs your Roblox game with a Git repository. Edit in Studio or VS Code - changes sync automatically in both directions.

## Core Features

| Feature | Description |
|---------|-------------|
| **True Two-Way Sync** | Automatic, real-time. No manual `syncback` commands. |
| **One-Click Extraction** | Extract any existing game to Git in seconds |
| **Sync All Instance Types** | Scripts, Parts, GUIs, Animations, Sounds, Attachments & more via `.rbxjson` |
| **Native AI Integration** | MCP support lets Claude/GPT control Studio directly |
| **E2E Testing** | Run playtests from CLI, stream console output in real-time |

---

# How is this different from Rojo?

Rojo is a great tool with a mature ecosystem. RbxSync takes a different approach:

| Feature | RbxSync | Rojo |
|---------|---------|------|
| **Two-way sync** | Automatic | Via syncback |
| **Game extraction** | One-click | Requires setup |
| **Property format** | JSON (`.rbxjson`) | XML / Binary |
| **AI / MCP integration** | Native | Not available |
| **E2E testing** | Built-in | Not available |

> **The key difference:** RbxSync is designed to work out of the box. Extract any game, start syncing immediately, optionally connect AI agents.

Rojo requires more initial setup but has years of ecosystem development behind it. Different tools for different workflows.

---

# Why `.rbxjson`?

RbxSync uses a JSON format with explicit type annotations:

```json
{
  "className": "Part",
  "properties": {
    "Size": { "type": "Vector3", "value": { "x": 4, "y": 1, "z": 2 } },
    "Anchored": { "type": "bool", "value": true }
  }
}
```

This format is:
- Human-readable and diffable in Git
- Parseable by AI/LLMs without ambiguity
- Preserves full type information (not lossy like some XML conversions)

---

# Getting Started

<div align="center">

**[https://rbxsync.dev/#install](https://rbxsync.dev/#install)**

</div>

---

# Free Beta

RbxSync is free during beta. I'm looking for feedback:
- Bug reports
- Feature requests
- Edge cases with property serialization

Drop issues on GitHub, in our support server or reply here.

---

# Links

<div align="center">

**Website:** [https://rbxsync.dev](https://rbxsync.dev)
**Discord:** [https://discord.gg/dURVrFVAEs](https://discord.gg/dURVrFVAEs)

</div>

---

Thanks for checking it out!
