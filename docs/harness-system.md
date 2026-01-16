# Harness System

> Multi-session AI game development for Roblox

The Harness System enables AI agents (like Claude) to build Roblox games incrementally across multiple sessions, maintaining context and tracking progress even when the AI has no memory of previous work.

---

## The Problem

When you're building a Roblox game with AI assistance across multiple sessions:

- **No memory** - Each new Claude session starts fresh with zero context
- **Lost progress** - You have to re-explain what features exist, what's done, what's blocked
- **Missing decisions** - Design decisions, architecture notes, and handoff context are lost
- **Manual tracking** - No structured way to track feature progress

## The Solution

The Harness System provides structured persistence for AI game development:

```
.rbxsync/harness/
├── game.yaml           # Game definition and architecture
├── features.yaml       # Feature registry with status tracking
└── sessions/           # Development session logs
    ├── abc123.yaml     # Session 1 - Combat system
    └── def456.yaml     # Session 2 - Inventory
```

Each session can read previous context, update feature status, and leave handoff notes for the next session.

---

## Quick Start

### 1. Initialize Harness

```bash
curl -X POST http://localhost:44755/harness/init \
  -H "Content-Type: application/json" \
  -d '{
    "projectDir": "/path/to/game",
    "gameName": "My RPG",
    "genre": "RPG",
    "description": "A fantasy adventure game"
  }'
```

This creates the `.rbxsync/harness/` directory structure.

### 2. Start a Session

```bash
curl -X POST http://localhost:44755/harness/session/start \
  -H "Content-Type: application/json" \
  -d '{
    "projectDir": "/path/to/game",
    "initialGoals": ["Implement inventory system", "Add item pickups"]
  }'
```

Returns a `sessionId` and context from previous sessions.

### 3. Track Features

```bash
curl -X POST http://localhost:44755/harness/feature/update \
  -H "Content-Type: application/json" \
  -d '{
    "projectDir": "/path/to/game",
    "name": "Inventory System",
    "status": "in_progress",
    "priority": "high",
    "tags": ["ui", "gameplay"],
    "notes": ["Using ReplicatedStorage for ItemData", "Max 20 slots"]
  }'
```

### 4. End Session with Handoff

```bash
curl -X POST http://localhost:44755/harness/session/end \
  -H "Content-Type: application/json" \
  -d '{
    "projectDir": "/path/to/game",
    "sessionId": "abc-123-def",
    "summary": "Completed basic inventory UI and item data structure",
    "handoffNotes": [
      "Need to add drag-drop functionality",
      "ItemData module is in ReplicatedStorage/Data",
      "Using RemoteEvents for client-server sync"
    ],
    "completedFeatures": ["Inventory System"],
    "blockers": []
  }'
```

### 5. Check Status (Next Session)

```bash
curl -X POST http://localhost:44755/harness/status \
  -H "Content-Type: application/json" \
  -d '{"projectDir": "/path/to/game"}'
```

Returns complete game state, all features, and recent session summaries.

---

## API Reference

### POST /harness/init

Initialize a new harness for a project.

**Request:**
```json
{
  "projectDir": "/path/to/game",
  "gameName": "My Game",
  "description": "Optional description",
  "genre": "RPG"
}
```

**Response:**
```json
{
  "success": true,
  "message": "Harness initialized",
  "harnessDir": "/path/to/game/.rbxsync/harness",
  "gameId": "uuid-here"
}
```

---

### POST /harness/session/start

Start a new development session.

**Request:**
```json
{
  "projectDir": "/path/to/game",
  "initialGoals": ["Goal 1", "Goal 2"]
}
```

**Response:**
```json
{
  "success": true,
  "sessionId": "uuid-here",
  "game": { /* GameDefinition */ },
  "features": [ /* Feature[] */ ],
  "previousSessions": [ /* Recent session summaries */ ],
  "suggestedNextSteps": ["Based on status..."]
}
```

---

### POST /harness/session/end

End the current session with summary and handoff notes.

**Request:**
```json
{
  "projectDir": "/path/to/game",
  "sessionId": "uuid-from-start",
  "summary": "What was accomplished",
  "handoffNotes": ["Note for next session"],
  "completedFeatures": ["Feature A"],
  "blockers": ["Blocked by X"]
}
```

---

### POST /harness/feature/update

Create or update a feature.

**Request:**
```json
{
  "projectDir": "/path/to/game",
  "name": "Feature Name",
  "description": "What it does",
  "status": "planned|in_progress|completed|blocked|cancelled",
  "priority": "critical|high|medium|low",
  "tags": ["ui", "gameplay"],
  "dependencies": ["Other Feature"],
  "acceptanceCriteria": ["Player can X", "System does Y"],
  "notes": ["Implementation notes"]
}
```

---

### POST /harness/status

Get current harness state.

**Request:**
```json
{
  "projectDir": "/path/to/game"
}
```

**Response:**
```json
{
  "success": true,
  "game": { /* GameDefinition */ },
  "features": {
    "total": 5,
    "byStatus": {
      "planned": 2,
      "in_progress": 1,
      "completed": 2,
      "blocked": 0
    },
    "list": [ /* Feature[] */ ]
  },
  "recentSessions": [ /* Last 5 sessions */ ],
  "nextSteps": ["Suggested actions"]
}
```

---

## Data Structures

### GameDefinition (game.yaml)

```yaml
id: "uuid"
name: "My RPG"
description: "A fantasy adventure game"
genre: "RPG"
targetAudience: "Casual gamers"
designGoals:
  - "Engaging combat system"
  - "Rich lore and story"
constraints:
  - "Must run on mobile"
  - "Max 50 concurrent players"
references:
  - "https://example.com/inspiration"
createdAt: "2026-01-16T12:00:00Z"
updatedAt: "2026-01-16T14:30:00Z"
```

### Feature (features.yaml)

```yaml
features:
  - id: "uuid"
    name: "Combat System"
    description: "Turn-based combat with abilities"
    status: completed
    priority: critical
    tags:
      - gameplay
      - core
    dependencies: []
    acceptanceCriteria:
      - "Player can attack enemies"
      - "Damage calculation works"
    notes:
      - "Using CombatModule in ServerScriptService"
    createdAt: "2026-01-15T10:00:00Z"
    updatedAt: "2026-01-16T14:00:00Z"
```

### SessionLog (sessions/\<id\>.yaml)

```yaml
id: "uuid"
startedAt: "2026-01-16T10:00:00Z"
endedAt: "2026-01-16T12:30:00Z"
initialGoals:
  - "Implement inventory system"
summary: "Completed basic inventory UI"
handoffNotes:
  - "Need drag-drop next"
  - "ItemData in ReplicatedStorage/Data"
entries:
  - timestamp: "2026-01-16T10:15:00Z"
    type: feature_started
    content: "Started Inventory System"
  - timestamp: "2026-01-16T12:00:00Z"
    type: feature_completed
    content: "Inventory UI done"
completedFeatures:
  - "Inventory System"
blockers: []
```

---

## Use Case: Building an RPG

### Session 1 (Monday)

```bash
# Initialize
curl -X POST localhost:44755/harness/init \
  -d '{"projectDir": "/games/rpg", "gameName": "Dragon Quest Clone", "genre": "RPG"}'

# Start session
curl -X POST localhost:44755/harness/session/start \
  -d '{"projectDir": "/games/rpg", "initialGoals": ["Set up project structure", "Combat system"]}'

# Create features
curl -X POST localhost:44755/harness/feature/update \
  -d '{"projectDir": "/games/rpg", "name": "Combat System", "status": "in_progress", "priority": "critical"}'

curl -X POST localhost:44755/harness/feature/update \
  -d '{"projectDir": "/games/rpg", "name": "Inventory", "status": "planned", "priority": "high"}'

curl -X POST localhost:44755/harness/feature/update \
  -d '{"projectDir": "/games/rpg", "name": "NPC Dialog", "status": "planned", "priority": "medium"}'

# ... work on combat ...

# Mark combat complete
curl -X POST localhost:44755/harness/feature/update \
  -d '{"projectDir": "/games/rpg", "name": "Combat System", "status": "completed"}'

# End session
curl -X POST localhost:44755/harness/session/end \
  -d '{
    "projectDir": "/games/rpg",
    "sessionId": "...",
    "summary": "Combat system complete with turn-based mechanics",
    "handoffNotes": [
      "Damage formula: ATK * 2 - DEF",
      "CombatModule in ServerScriptService/Combat",
      "Using BindableEvents for turn signals"
    ],
    "completedFeatures": ["Combat System"]
  }'
```

### Session 2 (Wednesday - Fresh Claude Instance)

```bash
# Check status - see what's done
curl -X POST localhost:44755/harness/status \
  -d '{"projectDir": "/games/rpg"}'

# Response shows:
# - Combat System: completed
# - Inventory: planned (suggested next)
# - Previous handoff: "Damage formula: ATK * 2 - DEF..."

# Start new session with full context
curl -X POST localhost:44755/harness/session/start \
  -d '{"projectDir": "/games/rpg", "initialGoals": ["Implement inventory"]}'

# Continue building with context preserved
```

---

## Feature Status Flow

```
planned → in_progress → completed
                ↓
              blocked → in_progress → completed
                ↓
            cancelled
```

---

## Best Practices

1. **Be specific in handoff notes** - Include file paths, module names, design decisions
2. **Use acceptance criteria** - Makes it clear when a feature is "done"
3. **Tag features consistently** - Helps filter and organize (ui, gameplay, networking, etc.)
4. **Track dependencies** - Know what needs to be done first
5. **End every session** - Even if incomplete, handoff notes help the next session

---

## Future Enhancements

- **MCP Tools** - Claude can query/update harness directly via tool calls
- **VS Code UI** - Visualize feature progress in sidebar
- **Game Templates** - Pre-built feature sets for common genres (Tycoon, Obby, Simulator)
- **Auto-context injection** - Harness state automatically included in Claude prompts
- **Feature verification** - Integration with bot testing to verify features work

---

## Related

- [MCP Integration](/mcp/) - AI tool integration
- [Bot Testing](/bot-testing) - Automated game testing
- [Getting Started](/getting-started/) - RbxSync basics
