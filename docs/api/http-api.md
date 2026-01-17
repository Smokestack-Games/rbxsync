# HTTP API Reference

RbxSync exposes an HTTP API for communication between the server, Roblox Studio plugin, VS Code extension, and MCP integrations.

## Overview

- **Default Port:** 44755
- **Default Host:** 127.0.0.1 (localhost only)
- **Base URL:** `http://127.0.0.1:44755`
- **Content-Type:** `application/json`
- **Max Body Size:** 10 MB

## Authentication

RbxSync does not implement authentication. The server binds to localhost by default, restricting access to the local machine only.

---

## Core Endpoints

### Health Check

Check if the server is running.

```
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "version": "1.3.0"
}
```

**curl example:**
```bash
curl http://127.0.0.1:44755/health
```

---

### Shutdown

Gracefully stop the server.

```
POST /shutdown
```

**Response:**
```json
{
  "status": "shutting_down"
}
```

**curl example:**
```bash
curl -X POST http://127.0.0.1:44755/shutdown
```

---

## Plugin Communication

These endpoints handle communication between the server and Roblox Studio plugin using a request/response pattern with long polling.

### Poll for Requests

Plugin polls this endpoint to receive commands from the server.

```
GET /rbxsync/request
```

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `projectDir` | string | (Optional) Project directory for project-specific commands |

**Response (200 OK):**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "command": "sync:create",
  "payload": { ... }
}
```

**Response (204 No Content):** No pending requests (timeout after 15s).

**curl example:**
```bash
curl "http://127.0.0.1:44755/rbxsync/request?projectDir=/path/to/project"
```

---

### Send Response

Plugin sends response to a request.

```
POST /rbxsync/response
```

**Request Body:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "success": true,
  "data": { ... },
  "error": null
}
```

**Response:**
```json
{
  "ok": true
}
```

---

### Register Place

Register a Studio place with the server.

```
POST /rbxsync/register
```

**Request Body:**
```json
{
  "place_id": 12345678,
  "place_name": "My Game",
  "project_dir": "/path/to/project",
  "session_id": "unique-session-id"
}
```

**Response:**
```json
{
  "success": true,
  "session_id": "unique-session-id"
}
```

---

### Unregister Place

Remove a Studio place from the registry.

```
POST /rbxsync/unregister
```

**Request Body:**
```json
{
  "session_id": "unique-session-id"
}
```

---

### Register VS Code Workspace

Register a VS Code workspace for project synchronization.

```
POST /rbxsync/register-vscode
```

**Request Body:**
```json
{
  "workspace_dir": "/path/to/workspace"
}
```

---

### List Connected Places

Get all registered Studio places.

```
GET /rbxsync/places
```

**Response:**
```json
{
  "places": [
    {
      "place_id": 12345678,
      "place_name": "My Game",
      "project_dir": "/path/to/project",
      "session_id": "unique-session-id"
    }
  ]
}
```

**curl example:**
```bash
curl http://127.0.0.1:44755/rbxsync/places
```

---

### List VS Code Workspaces

Get all registered VS Code workspaces.

```
GET /rbxsync/workspaces
```

**Response:**
```json
{
  "workspaces": ["/path/to/workspace1", "/path/to/workspace2"]
}
```

---

### Server Info

Get server information.

```
GET /rbxsync/server-info
```

**Response:**
```json
{
  "cwd": "/current/working/directory",
  "version": "1.3.0",
  "vscode_workspaces": ["/path/to/workspace"]
}
```

---

## Extraction Endpoints

These endpoints handle extracting a game from Roblox Studio to local files.

### Start Extraction

Begin a new extraction session.

```
POST /extract/start
```

**Request Body:**
```json
{
  "project_dir": "/path/to/project",
  "services": ["Workspace", "ReplicatedStorage", "ServerScriptService"],
  "include_terrain": true,
  "include_assets": true
}
```

**Response:**
```json
{
  "sessionId": "550e8400-e29b-41d4-a716-446655440000",
  "status": "started"
}
```

**curl example:**
```bash
curl -X POST http://127.0.0.1:44755/extract/start \
  -H "Content-Type: application/json" \
  -d '{"project_dir": "/my/project", "include_terrain": true}'
```

---

### Send Extraction Chunk

Plugin sends instance data in chunks.

```
POST /extract/chunk
```

**Request Body:**
```json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "chunk_index": 0,
  "total_chunks": 10,
  "data": [ ... ],
  "project_dir": "/path/to/project"
}
```

**Response:**
```json
{
  "received": 1,
  "total": 10
}
```

---

### Extraction Status

Check the status of an active extraction.

```
GET /extract/status
```

**Response:**
```json
{
  "status": "in_progress",
  "sessionId": "550e8400-e29b-41d4-a716-446655440000",
  "chunksReceived": 5,
  "totalChunks": 10
}
```

---

### Finalize Extraction

Write extracted data to files.

```
POST /extract/finalize
```

**Request Body:**
```json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "project_dir": "/path/to/project"
}
```

**Response:**
```json
{
  "success": true,
  "filesWritten": 245,
  "path": "/path/to/project/src"
}
```

---

### Extract Terrain

Send terrain data separately (can be batched).

```
POST /extract/terrain
```

**Request Body:**
```json
{
  "project_dir": "/path/to/project",
  "terrain": {
    "size": { "x": 512, "y": 512, "z": 512 },
    "chunks": [ ... ]
  },
  "batch_index": 1,
  "total_batches": 3
}
```

**Response:**
```json
{
  "success": true,
  "chunksWritten": 1024,
  "path": "/path/to/project/src/Workspace/Terrain.terrain.json"
}
```

---

## Sync Endpoints

These endpoints handle syncing local file changes to Roblox Studio.

### Sync Command

Send a single sync command to the plugin.

```
POST /sync/command
```

**Request Body:**
```json
{
  "command": "sync:create",
  "payload": {
    "path": "ServerScriptService/MyScript",
    "className": "ModuleScript",
    "properties": { ... }
  }
}
```

**Response:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "success": true,
  "data": { ... }
}
```

---

### Sync Batch

Send multiple sync operations in a single request.

```
POST /sync/batch
```

**Request Body:**
```json
{
  "operations": [
    { "type": "create", "path": "...", "data": { ... } },
    { "type": "update", "path": "...", "data": { ... } },
    { "type": "delete", "path": "..." }
  ]
}
```

**Timeout:** 5 minutes (for large batches)

---

### Read Tree

Read the instance tree from local files.

```
POST /sync/read-tree
```

**Request Body:**
```json
{
  "project_dir": "/path/to/project"
}
```

**Response:**
```json
{
  "success": true,
  "tree": [ ... ],
  "scripts": { ... }
}
```

---

### Read Terrain

Read terrain data from local files.

```
POST /sync/read-terrain
```

**Request Body:**
```json
{
  "project_dir": "/path/to/project"
}
```

---

### Sync From Studio

Handle changes from Studio and write to local files.

```
POST /sync/from-studio
```

**Request Body:**
```json
{
  "projectDir": "/path/to/project",
  "operations": [
    {
      "type": "modify",
      "path": "ServerScriptService/MyScript",
      "className": "ModuleScript",
      "data": { "source": "-- code here" }
    }
  ]
}
```

**Operation Types:**
- `create` - Create new instance/file
- `modify` - Update existing instance/file
- `delete` - Remove instance/file
- `rename` - Rename instance (uses `oldPath` and `newPath` in data)

---

### Incremental Sync

Sync only files changed since the last sync.

```
POST /sync/incremental
```

**Request Body:**
```json
{
  "project_dir": "/path/to/project"
}
```

---

## Diff Endpoints

### Get Studio Paths

Query instance paths from Studio.

```
POST /studio/paths
```

**Request Body:**
```json
{
  "services": ["Workspace", "ReplicatedStorage"]
}
```

---

### Diff

Compare local files with Studio state.

```
POST /diff
```

**Request Body:**
```json
{
  "project_dir": "/path/to/project"
}
```

**Response:**
```json
{
  "success": true,
  "onlyInFiles": ["path/to/new/file"],
  "onlyInStudio": ["path/in/studio/only"],
  "different": ["path/to/changed/file"]
}
```

---

## Git Endpoints

### Git Status

Get the git status of a project.

```
POST /git/status
```

**Request Body:**
```json
{
  "project_dir": "/path/to/project"
}
```

**Response:**
```json
{
  "success": true,
  "branch": "main",
  "staged": ["file1.luau"],
  "modified": ["file2.luau"],
  "untracked": ["file3.luau"]
}
```

---

### Git Log

Get recent commit history.

```
POST /git/log
```

**Request Body:**
```json
{
  "project_dir": "/path/to/project",
  "limit": 10
}
```

---

### Git Commit

Create a git commit.

```
POST /git/commit
```

**Request Body:**
```json
{
  "project_dir": "/path/to/project",
  "message": "feat: add new feature",
  "files": ["src/file.luau"]
}
```

---

### Git Init

Initialize a new git repository.

```
POST /git/init
```

**Request Body:**
```json
{
  "project_dir": "/path/to/project"
}
```

---

## Test Runner Endpoints

These endpoints control automated playtesting for E2E workflows.

### Start Test

Start a playtest in Studio.

```
POST /test/start
```

**Response:**
```json
{
  "success": true,
  "message": "Capture started"
}
```

---

### Test Status

Check playtest status.

```
GET /test/status
```

**Response:**
```json
{
  "running": true,
  "duration_seconds": 45
}
```

---

### Stop Test

Stop the current playtest.

```
POST /test/stop
```

---

## Bot Controller Endpoints

These endpoints enable AI-powered automated gameplay testing during playtests.

### Bot Command

Send a generic command to the bot.

```
POST /bot/command
```

**Request Body:**
```json
{
  "type": "move",
  "command": "walkTo",
  "args": { "position": { "x": 10, "y": 0, "z": 20 } }
}
```

---

### Bot Move

Move the character to a position or object.

```
POST /bot/move
```

**Request Body (position):**
```json
{
  "position": { "x": 10, "y": 0, "z": 20 }
}
```

**Request Body (object):**
```json
{
  "objectName": "SpawnPoint"
}
```

**Timeout:** 60 seconds

---

### Bot Action

Perform character actions.

```
POST /bot/action
```

**Request Body:**
```json
{
  "action": "equip",
  "name": "Sword"
}
```

**Actions:**
- `equip` - Equip a tool
- `unequip` - Unequip current tool
- `activate` - Activate equipped tool
- `deactivate` - Deactivate tool
- `interact` - Interact with an object
- `jump` - Make character jump

---

### Bot Observe

Get game state observations.

```
POST /bot/observe
```

**Request Body:**
```json
{
  "type": "nearby",
  "radius": 50,
  "query": "NPC"
}
```

**Observation Types:**
- `state` - Full game state (position, health, inventory)
- `nearby` - Objects within radius
- `npcs` - NPCs within radius
- `inventory` - Player inventory
- `find` - Search for specific objects

---

### Bot State

Get/update the latest bot state.

```
GET /bot/state
```

**Response:**
```json
{
  "position": { "x": 10, "y": 0, "z": 20 },
  "health": 100,
  "inventory": ["Sword", "Shield"]
}
```

```
POST /bot/state
```

Update state (called by running game).

---

### Bot Queue

Queue a command for the bot to execute.

```
POST /bot/queue
```

**Request Body:**
```json
{
  "type": "move",
  "position": { "x": 100, "y": 0, "z": 100 }
}
```

**Response:**
```json
{
  "success": true,
  "queued": true,
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "queue_length": 1
}
```

---

### Bot Pending

Get the next pending command (called by running game).

```
GET /bot/pending
```

**Response:**
```json
{
  "success": true,
  "command": { "type": "move", ... },
  "id": "550e8400-e29b-41d4-a716-446655440000"
}
```

---

### Bot Result

Submit/retrieve command results.

```
POST /bot/result
```

**Request Body:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "success": true,
  "data": { ... }
}
```

```
GET /bot/result/:id
```

Get result for a specific command ID.

---

### Bot Playtest Status

Check if a playtest is currently active.

```
GET /bot/playtest
```

**Response:**
```json
{
  "active": true,
  "duration_seconds": 120
}
```

---

### Bot Lifecycle

Handle bot lifecycle events.

```
POST /bot/lifecycle
```

**Request Body:**
```json
{
  "event": "hello",
  "reason": null
}
```

**Events:**
- `hello` - Bot connected (playtest started)
- `goodbye` - Bot disconnected (playtest ended)

---

## Console Streaming Endpoints

These endpoints provide console output streaming for E2E testing.

### Push Console Messages

Plugin pushes console messages to the server.

```
POST /console/push
```

**Request Body:**
```json
{
  "messages": [
    {
      "timestamp": "12:34:56",
      "message_type": "info",
      "message": "Game started",
      "source": "studio"
    }
  ]
}
```

**Message Types:** `info`, `warn`, `error`

---

### Console History

Get recent console messages.

```
GET /console/history
```

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `limit` | number | Max messages to return (default: 100, max: 1000) |

**Response:**
```json
{
  "messages": [ ... ],
  "total": 150
}
```

---

### Console Subscribe

Subscribe to real-time console output via Server-Sent Events.

```
GET /console/subscribe
```

**Response:** Server-Sent Events stream

**curl example:**
```bash
curl -N http://127.0.0.1:44755/console/subscribe
```

Each event contains a JSON console message:
```
data: {"timestamp":"12:34:56","message_type":"info","message":"Output text","source":"studio"}
```

---

## Run Code Endpoint

Execute arbitrary Luau code in Roblox Studio.

```
POST /run
```

**Request Body:**
```json
{
  "code": "return workspace:GetChildren()"
}
```

**Response:**
```json
{
  "success": true,
  "output": "[Part, Model, Script]",
  "error": null
}
```

**Timeout:** 30 seconds

**curl example:**
```bash
curl -X POST http://127.0.0.1:44755/run \
  -H "Content-Type: application/json" \
  -d '{"code": "print(\"Hello from RbxSync!\"); return 42"}'
```

---

## Error Responses

All endpoints return consistent error responses:

**Client Error (4xx):**
```json
{
  "success": false,
  "error": "Description of the error"
}
```

**Server Error (5xx):**
```json
{
  "success": false,
  "error": "Internal server error description"
}
```

**Timeout (504/408):**
```json
{
  "error": "Timeout waiting for plugin response"
}
```

---

## Common Status Codes

| Code | Meaning |
|------|---------|
| 200 | Success |
| 204 | No Content (empty response) |
| 400 | Bad Request |
| 408 | Request Timeout |
| 500 | Internal Server Error |
| 504 | Gateway Timeout |
