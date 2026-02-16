---
name: assign-agent
description: Launch delegated Codex workers in dedicated tmux sessions via codex-tmux, with resumable CODEX_THREAD_ID handoff for GitHub issue workflows.
---

# Assign Agent

Use this skill when an orchestrator agent needs to assign work to another local Codex agent.

## Purpose
- Start a worker in its own tmux session using `codex-tmux`.
- Keep worker sessions resumable by capturing `CODEX_THREAD_ID`.
- Standardize issue delegation prompts and handoff artifacts.

## Required launcher
- Launcher path: `/Users/robertsale/.bin/codex-tmux`
- Skill script path: `scripts/codex-tmux` (hard-linked copy of the launcher)

## Session naming
Use:
- `<repo-name>-<issue-number>-<brief-description>`

Examples:
- `coolproject-70-wa-geo-tax-lookup`
- `portal-144-auth-retry`

## Orchestrator launch rule
Always clear `CODEX_THREAD_ID` for new workers:

```bash
SESSION_NAME="<repo>-<issue>-<slug>"
PROMPT="<assignment prompt>"
CODEX_THREAD_ID= codex-tmux "$SESSION_NAME" "$PROMPT"
```

## Assignment prompt template
Seed the worker with enough context to begin immediately:

```text
You are assigned to <issue/reference> in <repo path or repo URL>.

Goals:
1. Research and triage the issue.
2. If [$gh-version-control-workflow](/Users/robertsale/Code/ezra/ezra/.codex/skills/gh-version-control-workflow/SKILL.md) is in your current skill list/context:
   - Add a GitHub issue comment that includes this exact fenced command:
     ```bash
     CODEX_THREAD_ID="$(printenv CODEX_THREAD_ID)" codex-tmux "$SESSION_NAME"
     ```
3. If that skill is not available in your current context:
   - Write a local resume command to `$WORKTREE_ROOT/codex.env`:
     ```bash
     cat > "$WORKTREE_ROOT/codex.env" <<EOF
     CODEX_THREAD_ID='$(printenv CODEX_THREAD_ID)' codex-tmux '$SESSION_NAME'
     EOF
     ```
4. Create the worktree/branch, implement the fix, open PR, and wait for review feedback.
```

## Worker behavior constraints
- One assignment maps to one worker session.
- Do not reuse an unrelated existing tmux session.
- Keep the session name stable for the life of the issue to preserve resume ergonomics.
