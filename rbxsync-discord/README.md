# RbxSync Discord Feedback Bot

A Discord bot for collecting user feedback and bug reports for RbxSync.

## Features

- **Slash Commands**: `/bug`, `/feedback`, `/feature` with modal forms
- **Auto-Capture**: Monitors designated channels for reports
- **Structured Reports**: Saves all reports as JSON files
- **Rich Embeds**: Formatted responses with report IDs

## Quick Start

### 1. Create a Discord Bot

1. Go to [Discord Developer Portal](https://discord.com/developers/applications)
2. Click "New Application" and give it a name
3. Go to "Bot" section and click "Add Bot"
4. Under "Privileged Gateway Intents", enable:
   - Message Content Intent
5. Copy the bot token

### 2. Invite Bot to Your Server

1. Go to "OAuth2" > "URL Generator"
2. Select scopes: `bot`, `applications.commands`
3. Select bot permissions:
   - Send Messages
   - Send Messages in Threads
   - Embed Links
   - Read Message History
   - Use Slash Commands
4. Copy the generated URL and open it in your browser
5. Select your server and authorize

### 3. Configure Environment

```bash
cd rbxsync-discord
cp .env.example .env
```

Edit `.env` with your bot token:

```env
DISCORD_TOKEN=your_bot_token_here
REPORTS_CHANNEL_ID=optional_channel_id_for_aggregated_reports
```

### 4. Install and Run

```bash
npm install
npm start
```

For development with auto-reload:

```bash
npm run dev
```

## Usage

### Slash Commands

| Command | Description |
|---------|-------------|
| `/bug` | Opens a bug report form |
| `/feedback` | Opens a feedback submission form |
| `/feature` | Opens a feature request form |

### Auto-Capture Channels

The bot automatically captures messages in channels named:

**Bug Reports:**
- `bug-reports`
- `bugs`
- `rbxsync-bugs`

**Feedback:**
- `feedback`
- `suggestions`
- `rbxsync-feedback`

### Report Storage

Reports are saved as JSON files in the `reports/` directory:

```
reports/
  bug-RPT-XXXXXX-YYYY.json
  feedback-RPT-XXXXXX-YYYY.json
  feature-RPT-XXXXXX-YYYY.json
```

Each report includes:
- Unique report ID
- Reporter info (Discord ID and tag)
- Timestamp
- Full report content
- Status tracking

## Report Format

### Bug Report

```json
{
  "id": "RPT-LXYZ1234-AB12",
  "type": "bug",
  "title": "Sync fails on large projects",
  "description": "When syncing a project with 500+ scripts...",
  "stepsToReproduce": "1. Open large project\n2. Run sync",
  "version": "v1.3.0",
  "reporter": {
    "id": "123456789",
    "tag": "user#1234"
  },
  "createdAt": "2026-01-16T12:00:00.000Z",
  "status": "open"
}
```

### Feedback Report

```json
{
  "id": "RPT-LXYZ1234-AB12",
  "type": "feedback",
  "feedbackType": "UX",
  "content": "The extraction process could show more progress...",
  "reporter": {
    "id": "123456789",
    "tag": "user#1234"
  },
  "createdAt": "2026-01-16T12:00:00.000Z",
  "status": "open"
}
```

### Feature Request

```json
{
  "id": "RPT-LXYZ1234-AB12",
  "type": "feature",
  "title": "Support for .rbxmx files",
  "description": "Add ability to import/export .rbxmx format",
  "useCase": "Would help with asset sharing between projects",
  "reporter": {
    "id": "123456789",
    "tag": "user#1234"
  },
  "createdAt": "2026-01-16T12:00:00.000Z",
  "status": "open"
}
```

## Configuration Options

| Environment Variable | Required | Description |
|---------------------|----------|-------------|
| `DISCORD_TOKEN` | Yes | Your Discord bot token |
| `REPORTS_CHANNEL_ID` | No | Channel ID to aggregate all reports |

## Development

### Project Structure

```
rbxsync-discord/
  src/
    index.js      # Main bot code
  reports/        # Saved reports (gitignored)
  .env            # Environment config (gitignored)
  .env.example    # Example environment config
  package.json    # Dependencies
  README.md       # This file
```

### Adding New Report Types

1. Add a new slash command in the `commands` array
2. Create a modal handler in `handleSlashCommand()`
3. Add processing logic in `handleModalSubmit()`
4. Create a report factory function (e.g., `createXxxReport()`)

## Troubleshooting

### Bot not responding to commands

- Ensure the bot has proper permissions in your server
- Check that slash commands are registered (wait a few minutes after first run)
- Verify `DISCORD_TOKEN` is correct

### Reports not saving

- Check that the `reports/` directory is writable
- Look for errors in the console output

### Missing Message Content Intent

If auto-capture doesn't work, enable "Message Content Intent" in the Discord Developer Portal under your bot's settings.

## License

MIT - Part of the RbxSync project
