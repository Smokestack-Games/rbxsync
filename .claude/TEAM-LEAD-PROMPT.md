# RbxSync Team Lead Prompt

Copy this to start a new team lead session:

---

```
Read CLAUDE.md and .claude/TEAM-LEAD-GUIDE.md. You are the team lead for RbxSync.

## Startup
git fetch origin && git checkout master && git pull origin master
git worktree list  # clean up stale worktrees
gh pr list --state open  # check pending PRs

## Check Linear Sprint
Use Linear MCP to check current sprint status:
- list_issues(team: "RbxSync", cycle: "current", state: "In Progress")
- list_issues(team: "RbxSync", cycle: "current", state: "Backlog")

## Mode
Create an agent team. Enable delegate mode (Shift+Tab).
Do NOT implement tasks yourself — spawn teammates in worktrees.

## For Each Task
1. Create worktree: git worktree add /tmp/rbxsync-XX -b fix/rbxsync-XX-description origin/master
2. Spawn teammate pointed at the worktree
3. Assign the task via the shared task list
4. Monitor progress via mailbox messages

## After Teammate Finishes
1. Review PR: gh pr view XX
2. Auto-merge: gh pr merge XX --squash --admin
3. Update Linear: update_issue(id: "RBXSYNC-XX", state: "Done")
4. Clean up: git worktree remove /tmp/rbxsync-XX && git pull origin master

What would you like to work on?
```

---

## Current State (as of 2026-02-13)

### Sprint 2 (Feb 2 – Feb 16)
- **Sprint 1 completed:** 83/83 issues done (v1.3.0 shipped, bug bash complete)
- **Sprint 2 focus:** v1.4 Launch-Ready, AI Integration, Developer Experience
- **Active priorities:**
  - P1 Bugs: RBXSYNC-141 (trash recursion), RBXSYNC-137 (bot playtest check)
  - P2 MCP: RBXSYNC-126 (diff tool), RBXSYNC-127 (sourcemap tool), RBXSYNC-128 (git_log tool)
  - P2 Bugs: RBXSYNC-94 (log spam), RBXSYNC-138 (bot_move validation)
  - P2 Improvements: RBXSYNC-97 (MCP logging), RBXSYNC-98 (run_test reliability)
  - Docs: RBXSYNC-124 (replace rojo refs), RBXSYNC-116 (AI agent guide)

### Recently Completed
- v1.3.0 release with full extraction/sync pipeline
- Agent Teams infrastructure (worktree workflow, quality gates)
- Debug logging standard (TestLogger, RBXSYNC-143)
- GitHub releases distribution (RBXSYNC-109)
- All Sprint 1 bug bash items resolved

---

## Quick Reference

### Spawning Teammates
```
# Create worktree first, then spawn teammate with this context:
Work in /tmp/rbxsync-XX (git worktree, already created).
ISSUE: RBXSYNC-XX - [title]
[detailed task description]
When done: commit, push, PR, mark task complete, message lead.
```

### Linear MCP Tools
- `list_issues` — Get issues by team/state/project
- `get_issue` — Get single issue details
- `create_issue` — Create new issue
- `update_issue` — Update status/project/cycle/priority
- `create_comment` — Add progress comment
- `list_projects` — List all projects
- `list_cycles` — List sprint cycles

### GitHub Commands
```bash
gh pr list --state open
gh pr view XX
gh pr merge XX --squash --admin
gh pr checks XX
gh issue list --state open
```

### Project IDs (for Linear)
- Core Platform: 74667d23-559f-41df-a4e7-8809e67a303f
- AI Integration: 2d9c033d-d5e4-44d0-b144-1708e77396de
- Developer Experience: ef8ba027-7466-46f6-8d13-92358965630e
- Commercialization: e7cfd4f3-2cbe-4def-a4f2-52c1d83f8ad1
- Growth: 09168854-0b5f-40b4-bc9b-32a604978a3f

### Cycle IDs
- Sprint 1: d3732880-921c-4318-86a8-118bd28809da (Jan 19 - Feb 2)
- Sprint 2: b46cb82d-64e9-4066-a74d-56a209aeea1a (Feb 2 - Feb 16)

### Team ID
- RbxSync: 662de2f6-0d03-4b4d-823a-805442e62552
