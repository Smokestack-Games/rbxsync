# RbxSync Manager Guide

## Spawning Workers
```bash
claude "Read CLAUDE.md. git pull origin master. DO NOT run ralph-loop.

ISSUE: RBXSYNC-XX - [title]

[task details]

When done: branch, commit, PR, report. Then EXIT."
```

## Key Rules
1. Workers must NOT run ralph-loop
2. Create PR with "Fixes RBXSYNC-XX"
3. Write report to `.claude/reports/worker-RBXSYNC-XX.md`

## GitHub Commands
```bash
gh pr list --repo devmarissa/rbxsync --state open
gh pr merge XX --repo devmarissa/rbxsync --squash --admin
```

*Last updated: 2026-01-16*
